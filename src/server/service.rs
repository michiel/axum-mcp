//! Core MCP server implementation

use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::{
    error::{McpError, McpResult},
    protocol::{
        BatchItemResult, BatchParams, BatchResult, InitializeParams, JsonRpcRequest,
        JsonRpcResponse, StandardMethod, ToolsCallParams, ToolsListResult,
    },
    security::SecurityContext,
    server::{
        config::McpServerConfig,
        progress::{ProgressReporter, ProgressUpdate},
        registry::{ToolExecutionContext, ToolRegistry},
        BatchContext, McpServerState, ServerHealth,
    },
};

/// Core MCP server implementation
#[derive(Clone)]
pub struct McpServer<S>
where
    S: McpServerState,
{
    /// Server configuration
    config: McpServerConfig,

    /// Server state implementation
    state: S,

    /// Progress reporter for long-running operations
    progress_reporter: Arc<ProgressReporter>,

    /// Server health information
    health: Arc<RwLock<ServerHealth>>,

    /// Active connections counter
    active_connections: Arc<RwLock<usize>>,

    /// Server start time
    start_time: std::time::Instant,
}

impl<S> McpServer<S>
where
    S: McpServerState,
{
    /// Create a new MCP server with the given state
    pub fn new(config: McpServerConfig, state: S) -> Self {
        Self {
            config,
            state,
            progress_reporter: Arc::new(ProgressReporter::new()),
            health: Arc::new(RwLock::new(ServerHealth::default())),
            active_connections: Arc::new(RwLock::new(0)),
            start_time: std::time::Instant::now(),
        }
    }

    /// Get server configuration
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Get server state
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get progress reporter
    pub fn progress_reporter(&self) -> Arc<ProgressReporter> {
        Arc::clone(&self.progress_reporter)
    }

    /// Handle an MCP JSON-RPC request
    pub fn handle_request(
        &self,
        request: JsonRpcRequest,
        context: SecurityContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = JsonRpcResponse> + Send + '_>> {
        Box::pin(async move {
            debug!(
                "Handling MCP request: {} (id: {:?})",
                request.method, request.id
            );

            // Authenticate and authorize the request
            if let Err(error) = self.validate_request(&request, &context).await {
                return JsonRpcResponse::error(error.into(), request.id);
            }

            // Parse the method
            let method = match self.parse_method(&request.method) {
                Ok(method) => method,
                Err(error) => {
                    return JsonRpcResponse::error(error.into(), request.id);
                }
            };

            // Handle the request based on method type
            let result = match method {
                InternalMcpMethod::Standard(standard_method) => {
                    self.handle_standard_method(standard_method, request.params, &context)
                        .await
                }
                InternalMcpMethod::Custom(custom_method) => {
                    self.state
                        .handle_custom_method(&custom_method, request.params, &context)
                        .await
                }
            };

            // Convert result to JSON-RPC response
            match result {
                Ok(Some(value)) => JsonRpcResponse::success(value, request.id),
                Ok(None) => JsonRpcResponse::success(serde_json::Value::Null, request.id),
                Err(error) => {
                    error!("Request failed: {} - {}", request.method, error);
                    JsonRpcResponse::error(error.into(), request.id)
                }
            }
        })
    }

    /// Handle a standard MCP method
    async fn handle_standard_method(
        &self,
        method: StandardMethod,
        params: Option<serde_json::Value>,
        context: &SecurityContext,
    ) -> McpResult<Option<serde_json::Value>> {
        match method {
            StandardMethod::Initialize => {
                let init_params: InitializeParams = if let Some(params) = params {
                    serde_json::from_value(params).map_err(|e| McpError::Protocol {
                        message: format!("Invalid initialize params: {}", e),
                    })?
                } else {
                    return Err(McpError::Protocol {
                        message: "Initialize requires parameters".to_string(),
                    });
                };

                let result = self.state.initialize(init_params).await?;
                Ok(Some(serde_json::to_value(result)?))
            }

            StandardMethod::Initialized => {
                // Notification - no response needed
                info!("Client initialized successfully");
                Ok(None)
            }

            StandardMethod::NotificationsInitialized => {
                // Alternative initialization notification - no response needed
                info!("Client initialized successfully (via notifications/initialized)");
                Ok(None)
            }

            StandardMethod::Ping => {
                // Simple ping/pong for health checking
                Ok(Some(serde_json::json!({"status": "pong"})))
            }

            StandardMethod::ToolsList => {
                let tools = self.state.tool_registry().list_tools(context).await?;
                let result = ToolsListResult {
                    tools,
                    next_cursor: None,
                };
                Ok(Some(serde_json::to_value(result)?))
            }

            StandardMethod::ToolsCall => {
                let call_params: ToolsCallParams = if let Some(params) = params {
                    serde_json::from_value(params).map_err(|e| McpError::Protocol {
                        message: format!("Invalid tools/call params: {}", e),
                    })?
                } else {
                    return Err(McpError::Protocol {
                        message: "tools/call requires parameters".to_string(),
                    });
                };

                let execution_context = ToolExecutionContext::new(context.clone())
                    .with_arguments(call_params.arguments.unwrap_or(serde_json::Value::Null));

                let result = self
                    .state
                    .tool_registry()
                    .execute_tool(&call_params.name, execution_context)
                    .await?;

                Ok(Some(serde_json::to_value(result)?))
            }

            StandardMethod::Batch => {
                if !self.config.enable_batch {
                    return Err(McpError::Protocol {
                        message: "Batch operations are not enabled".to_string(),
                    });
                }

                let batch_params: BatchParams = if let Some(params) = params {
                    serde_json::from_value(params).map_err(|e| McpError::Protocol {
                        message: format!("Invalid batch params: {}", e),
                    })?
                } else {
                    return Err(McpError::Protocol {
                        message: "batch requires parameters".to_string(),
                    });
                };

                let result = self.handle_batch_request(batch_params, context).await?;
                Ok(Some(serde_json::to_value(result)?))
            }

            StandardMethod::ResourcesList => {
                if let Some(resource_registry) = self.state.resource_registry() {
                    let templates = resource_registry.list_resource_templates(context).await?;
                    let result = crate::protocol::ResourcesListResult {
                        resources: templates
                            .into_iter()
                            .map(|template| crate::protocol::messages::Resource {
                                uri: template.uri_template,
                                name: template.name,
                                description: template.description,
                                mime_type: template.mime_type,
                                metadata: template.metadata,
                            })
                            .collect(),
                        next_cursor: None,
                    };
                    Ok(Some(serde_json::to_value(result)?))
                } else {
                    Err(McpError::Protocol {
                        message: "Resources not supported by this server".to_string(),
                    })
                }
            }

            StandardMethod::ResourcesRead => {
                if let Some(resource_registry) = self.state.resource_registry() {
                    let read_params: crate::protocol::ResourcesReadParams =
                        if let Some(params) = params {
                            serde_json::from_value(params).map_err(|e| McpError::Protocol {
                                message: format!("Invalid resources/read params: {}", e),
                            })?
                        } else {
                            return Err(McpError::Protocol {
                                message: "resources/read requires parameters".to_string(),
                            });
                        };

                    let resource = resource_registry
                        .get_resource(&read_params.uri, context)
                        .await?;

                    // Convert ResourceContent from server to protocol
                    let protocol_content = match resource.content {
                        crate::server::resource::ResourceContent::Text { text } => {
                            crate::protocol::messages::ResourceContent::Text {
                                text,
                                uri: resource.uri.clone(),
                                mime_type: resource.mime_type,
                            }
                        }
                        crate::server::resource::ResourceContent::Blob { blob, mime_type } => {
                            crate::protocol::messages::ResourceContent::Blob {
                                blob,
                                uri: resource.uri.clone(),
                                mime_type,
                            }
                        }
                    };

                    let result = crate::protocol::ResourcesReadResult {
                        contents: vec![protocol_content],
                    };
                    Ok(Some(serde_json::to_value(result)?))
                } else {
                    Err(McpError::Protocol {
                        message: "Resources not supported by this server".to_string(),
                    })
                }
            }

            StandardMethod::PromptsList => {
                if let Some(prompt_registry) = self.state.prompt_registry() {
                    let prompts = prompt_registry.list_prompts(context).await?;
                    // For now, return a simple JSON response since the framework doesn't have PromptsListResult
                    let result = serde_json::json!({
                        "prompts": prompts.into_iter().map(|prompt| {
                            serde_json::json!({
                                "name": prompt.name,
                                "description": prompt.description,
                                "arguments": prompt.parameters.into_iter().map(|param| {
                                    serde_json::json!({
                                        "name": param.name,
                                        "description": param.description,
                                        "required": param.required,
                                        "schema": param.schema
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }).collect::<Vec<_>>()
                    });
                    Ok(Some(result))
                } else {
                    Err(McpError::Protocol {
                        message: "Prompts not supported by this server".to_string(),
                    })
                }
            }

            StandardMethod::PromptsGet => {
                if let Some(prompt_registry) = self.state.prompt_registry() {
                    let get_params: crate::server::prompt::GetPromptRequest =
                        if let Some(params) = params {
                            serde_json::from_value(params).map_err(|e| McpError::Protocol {
                                message: format!("Invalid prompts/get params: {}", e),
                            })?
                        } else {
                            return Err(McpError::Protocol {
                                message: "prompts/get requires parameters".to_string(),
                            });
                        };

                    let result = prompt_registry
                        .get_prompt_with_args(get_params, context)
                        .await?;
                    Ok(Some(serde_json::to_value(result)?))
                } else {
                    Err(McpError::Protocol {
                        message: "Prompts not supported by this server".to_string(),
                    })
                }
            }

            _ => {
                // Other methods not yet implemented
                Err(McpError::ToolNotFound {
                    name: format!("{:?}", method),
                })
            }
        }
    }

    /// Handle a batch request
    async fn handle_batch_request(
        &self,
        batch: BatchParams,
        context: &SecurityContext,
    ) -> McpResult<BatchResult> {
        if batch.requests.len() > self.config.max_batch_size {
            return Err(McpError::Validation {
                message: format!(
                    "Batch size {} exceeds maximum {}",
                    batch.requests.len(),
                    self.config.max_batch_size
                ),
            });
        }

        let batch_context = BatchContext {
            mode: match batch.execution_mode {
                crate::protocol::BatchExecutionMode::Parallel => {
                    crate::server::BatchExecutionMode::Parallel
                }
                crate::protocol::BatchExecutionMode::Sequential => {
                    crate::server::BatchExecutionMode::Sequential
                }
                crate::protocol::BatchExecutionMode::Dependency => {
                    crate::server::BatchExecutionMode::Sequential
                }
                crate::protocol::BatchExecutionMode::PriorityDependency => {
                    crate::server::BatchExecutionMode::Sequential
                }
            },
            max_parallel: batch.max_parallel.map(|v| v as usize),
            timeout: batch.timeout_ms.map(std::time::Duration::from_millis),
            security: context.clone(),
        };

        let progress_id = uuid::Uuid::new_v4().to_string();
        let total_items = batch.requests.len();

        // Send initial progress update
        self.progress_reporter
            .report_progress(ProgressUpdate::started(
                progress_id.clone(),
                "Processing batch request".to_string(),
                total_items,
            ))
            .await;

        let batch_requests = batch.requests.clone();
        let results = match batch_context.mode {
            crate::server::BatchExecutionMode::Parallel => {
                self.execute_batch_parallel(batch_requests, &batch_context, &progress_id)
                    .await
            }
            crate::server::BatchExecutionMode::Sequential
            | crate::server::BatchExecutionMode::FailFast => {
                self.execute_batch_sequential(batch_requests, &batch_context, &progress_id)
                    .await
            }
        };

        // Send completion progress update
        self.progress_reporter
            .report_progress(ProgressUpdate::completed(
                progress_id,
                "Batch request completed".to_string(),
            ))
            .await;

        let successful_count = results.iter().filter(|r| r.error.is_none()).count() as u32;
        let failed_count = results.iter().filter(|r| r.error.is_some()).count() as u32;

        Ok(BatchResult {
            stats: crate::protocol::BatchStats {
                total_requests: total_items as u32,
                successful_requests: successful_count,
                failed_requests: failed_count,
                skipped_requests: 0,
                total_execution_time_ms: 0, // TODO: Calculate actual time
                average_execution_time_ms: 0.0, // TODO: Calculate actual time
                max_parallel_executed: batch_context.max_parallel.unwrap_or(1) as u32,
            },
            results,
            correlation_token: None,
            metadata: std::collections::HashMap::new(),
        })
    }

    /// Execute batch items in parallel
    async fn execute_batch_parallel(
        &self,
        items: Vec<crate::protocol::BatchRequest>,
        context: &BatchContext,
        progress_id: &str,
    ) -> Vec<BatchItemResult> {
        use futures_util::stream::{self, StreamExt};

        let max_parallel = context.max_parallel.unwrap_or(10).min(items.len());
        let mut completed = 0;

        stream::iter(items)
            .map(|item| async move {
                // Prevent batch requests within batch requests to avoid recursion
                if item.method == "batch" {
                    completed += 1;

                    // Report progress
                    self.progress_reporter
                        .report_progress(ProgressUpdate::progress(
                            progress_id.to_string(),
                            format!("Processed {} items", completed),
                            completed,
                        ))
                        .await;

                    return BatchItemResult {
                        id: item.id,
                        result: None,
                        error: Some(crate::protocol::JsonRpcError {
                            code: -32600,
                            message: "Nested batch requests are not allowed".to_string(),
                            data: None,
                        }),
                        execution_time_ms: 0,
                        skipped: false,
                        metadata: HashMap::new(),
                    };
                }

                let json_rpc_request = crate::protocol::JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    method: item.method.clone(),
                    params: item.params.clone(),
                    id: Some(serde_json::Value::String(item.id.clone())),
                };
                let result = self
                    .handle_request(json_rpc_request, context.security.clone())
                    .await;
                completed += 1;

                // Report progress
                self.progress_reporter
                    .report_progress(ProgressUpdate::progress(
                        progress_id.to_string(),
                        format!("Processed {} items", completed),
                        completed,
                    ))
                    .await;

                BatchItemResult {
                    id: item.id,
                    result: if result.error.is_none() {
                        result.result
                    } else {
                        None
                    },
                    error: result.error,
                    execution_time_ms: 0, // TODO: Add timing
                    skipped: false,
                    metadata: HashMap::new(),
                }
            })
            .buffer_unordered(max_parallel)
            .collect()
            .await
    }

    /// Execute batch items sequentially
    async fn execute_batch_sequential(
        &self,
        items: Vec<crate::protocol::BatchRequest>,
        context: &BatchContext,
        progress_id: &str,
    ) -> Vec<BatchItemResult> {
        let mut results = Vec::with_capacity(items.len());
        let stop_on_error = context
            .security
            .client
            .metadata
            .get("stop_on_error")
            .map(|v| v.as_str() == "true")
            .unwrap_or(false);

        for (index, item) in items.into_iter().enumerate() {
            // Prevent batch requests within batch requests to avoid recursion
            if item.method == "batch" {
                let batch_result = BatchItemResult {
                    id: item.id,
                    result: None,
                    error: Some(crate::protocol::JsonRpcError {
                        code: -32600,
                        message: "Nested batch requests are not allowed".to_string(),
                        data: None,
                    }),
                    execution_time_ms: 0,
                    skipped: false,
                    metadata: HashMap::new(),
                };
                results.push(batch_result);

                // Report progress
                self.progress_reporter
                    .report_progress(ProgressUpdate::progress(
                        progress_id.to_string(),
                        format!("Processed {} items", index + 1),
                        index + 1,
                    ))
                    .await;

                continue;
            }

            let json_rpc_request = crate::protocol::JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: item.method.clone(),
                params: item.params.clone(),
                id: Some(serde_json::Value::String(item.id.clone())),
            };
            let result = self
                .handle_request(json_rpc_request, context.security.clone())
                .await;

            let batch_result = BatchItemResult {
                id: item.id,
                result: if result.error.is_none() {
                    result.result
                } else {
                    None
                },
                error: result.error.clone(),
                execution_time_ms: 0, // TODO: Add timing
                skipped: false,
                metadata: HashMap::new(),
            };

            results.push(batch_result);

            // Report progress
            self.progress_reporter
                .report_progress(ProgressUpdate::progress(
                    progress_id.to_string(),
                    format!("Processed {} items", index + 1),
                    index + 1,
                ))
                .await;

            // Stop on first error if stop_on_error is true
            if stop_on_error && result.error.is_some() {
                break;
            }
        }

        results
    }

    /// Validate a request
    async fn validate_request(
        &self,
        request: &JsonRpcRequest,
        context: &SecurityContext,
    ) -> McpResult<()> {
        // Check if method requires initialization
        if let Ok(InternalMcpMethod::Standard(method)) = self.parse_method(&request.method) {
            if method.requires_initialization() && !context.is_authenticated() {
                // For simplicity, we'll consider system context as "initialized"
                // Real implementations might track initialization state
                if !context.is_system() && !context.has_capability("initialized") {
                    return Err(McpError::Protocol {
                        message: "Client must call initialize first".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Parse a method string into an MCP method
    fn parse_method(&self, method: &str) -> McpResult<InternalMcpMethod> {
        // Try to parse as standard method first
        if let Ok(standard_method) =
            serde_json::from_value::<StandardMethod>(serde_json::Value::String(method.to_string()))
        {
            Ok(InternalMcpMethod::Standard(standard_method))
        } else {
            // Treat as custom method
            Ok(InternalMcpMethod::Custom(method.to_string()))
        }
    }

    /// Get current server health
    pub async fn get_health(&self) -> ServerHealth {
        let mut health = self.health.read().await.clone();
        health.uptime_seconds = self.start_time.elapsed().as_secs();
        health.active_connections = *self.active_connections.read().await;
        health
    }

    /// Update server health status
    pub async fn update_health(&self, healthy: bool, status: String) {
        let mut health = self.health.write().await;
        health.healthy = healthy;
        health.status = status;
    }

    /// Increment active connections
    pub async fn connection_opened(&self) {
        let mut connections = self.active_connections.write().await;
        *connections += 1;
    }

    /// Decrement active connections
    pub async fn connection_closed(&self) {
        let mut connections = self.active_connections.write().await;
        if *connections > 0 {
            *connections -= 1;
        }
    }
}

/// MCP method enumeration
#[derive(Debug, Clone)]
enum InternalMcpMethod {
    /// Standard MCP protocol method
    Standard(StandardMethod),
    /// Custom application-specific method
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        security::{McpAuth, SecurityContext},
        server::registry::InMemoryToolRegistry,
    };
    use async_trait::async_trait;

    // Test server state implementation
    #[derive(Clone)]
    struct TestServerState {
        tools: InMemoryToolRegistry,
        auth: TestAuth,
    }

    #[derive(Clone)]
    struct TestAuth;

    #[async_trait]
    impl McpAuth for TestAuth {
        async fn authenticate(
            &self,
            _client_info: &crate::security::ClientContext,
        ) -> McpResult<SecurityContext> {
            Ok(SecurityContext::system())
        }

        async fn authorize(
            &self,
            _context: &SecurityContext,
            _resource: &str,
            _action: &str,
        ) -> bool {
            true
        }
    }

    impl McpServerState for TestServerState {
        type ToolRegistry = InMemoryToolRegistry;
        type AuthManager = TestAuth;

        fn tool_registry(&self) -> &Self::ToolRegistry {
            &self.tools
        }

        fn auth_manager(&self) -> &Self::AuthManager {
            &self.auth
        }
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = McpServerConfig::default();
        let state = TestServerState {
            tools: InMemoryToolRegistry::new(),
            auth: TestAuth,
        };

        let server = McpServer::new(config, state);
        assert_eq!(server.config().name, "MCP Server");

        let health = server.get_health().await;
        assert!(health.healthy);
        assert_eq!(health.active_connections, 0);
    }

    #[tokio::test]
    async fn test_ping_request() {
        let config = McpServerConfig::default();
        let state = TestServerState {
            tools: InMemoryToolRegistry::new(),
            auth: TestAuth,
        };

        let server = McpServer::new(config, state);
        let context = SecurityContext::system();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id: Some(serde_json::Value::String("test".to_string())),
        };

        let response = server.handle_request(request, context).await;
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }
}
