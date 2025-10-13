//! Prompt registry example with AI workflow templates
//!
//! This example demonstrates how to create and use prompt registries for
//! reusable AI interaction patterns and workflow templates.
//!
//! Run with: cargo run --example prompt_registry_example

use axum_mcp::{
    axum_integration::{mcp_routes_with_wrapper, McpServerWrapper},
    prelude::*,
    protocol::ServerInfo,
    server::{
        config::McpServerConfig,
        prompt::{
            EmbeddedResource, InMemoryPromptRegistry, PromptCategory, PromptParameter,
            ResourceAnnotation,
        },
        resource::{InMemoryResourceRegistry, Resource, ResourceContent, UriSchemeConfig},
        service::McpServer,
    },
    PromptRegistry, ResourceRegistry,
};
use std::collections::HashMap;
use tokio::net::TcpListener;

// Define server state with both resource and prompt registries
#[derive(Clone)]
struct AIWorkflowServerState {
    tools: InMemoryToolRegistry,
    auth: SimpleAuth,
    resources: InMemoryResourceRegistry,
    prompts: InMemoryPromptRegistry,
}

// Simple authentication that allows everything
#[derive(Clone)]
struct SimpleAuth;

#[async_trait]
impl McpAuth for SimpleAuth {
    async fn authenticate(&self, _client_info: &ClientContext) -> McpResult<SecurityContext> {
        Ok(SecurityContext::system())
    }

    async fn authorize(&self, _context: &SecurityContext, _resource: &str, _action: &str) -> bool {
        true
    }
}

impl McpServerState for AIWorkflowServerState {
    type ToolRegistry = InMemoryToolRegistry;
    type AuthManager = SimpleAuth;

    fn tool_registry(&self) -> &Self::ToolRegistry {
        &self.tools
    }

    fn auth_manager(&self) -> &Self::AuthManager {
        &self.auth
    }

    fn resource_registry(&self) -> Option<&dyn ResourceRegistry> {
        Some(&self.resources)
    }

    fn prompt_registry(&self) -> Option<&dyn PromptRegistry> {
        Some(&self.prompts)
    }

    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            name: "AI Workflow Server".to_string(),
            version: "1.0.0".to_string(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

fn create_ratchet_resources() -> InMemoryResourceRegistry {
    let ratchet_scheme = UriSchemeConfig::new("ratchet", "Ratchet task management")
        .with_types(vec!["task".to_string(), "execution".to_string()]);

    let mut registry = InMemoryResourceRegistry::new(ratchet_scheme);

    // Add example task definition
    registry.add_resource(Resource {
        uri: "ratchet://tasks/data-processor".to_string(),
        name: "Data Processing Task".to_string(),
        description: Some("A task that processes CSV data files".to_string()),
        mime_type: Some("application/json".to_string()),
        content: ResourceContent::Text {
            text: r#"{
  "name": "data-processor",
  "description": "Process CSV files and extract insights",
  "config": {
    "input_format": "csv",
    "output_format": "json",
    "transformations": ["clean_nulls", "normalize_columns", "calculate_stats"]
  },
  "schedule": "0 2 * * *"
}"#
            .to_string(),
        },
        metadata: HashMap::new(),
    });

    // Add example code file
    registry.add_resource(Resource {
        uri: "ratchet://code/validator.py".to_string(),
        name: "Data Validator Script".to_string(),
        description: Some("Python script for data validation".to_string()),
        mime_type: Some("text/x-python".to_string()),
        content: ResourceContent::Text {
            text: r#"import pandas as pd
import numpy as np
from typing import Dict, List, Any

class DataValidator:
    """Validates data quality and consistency."""
    
    def __init__(self, rules: Dict[str, Any]):
        self.rules = rules
        self.errors = []
    
    def validate_dataframe(self, df: pd.DataFrame) -> bool:
        """Validate a pandas DataFrame against defined rules."""
        self.errors = []
        
        # Check for required columns
        required_cols = self.rules.get('required_columns', [])
        missing_cols = [col for col in required_cols if col not in df.columns]
        if missing_cols:
            self.errors.append(f"Missing required columns: {missing_cols}")
        
        # Check data types
        type_rules = self.rules.get('column_types', {})
        for col, expected_type in type_rules.items():
            if col in df.columns and not df[col].dtype == expected_type:
                self.errors.append(f"Column {col} has type {df[col].dtype}, expected {expected_type}")
        
        # Check for null values where not allowed
        no_null_cols = self.rules.get('no_null_columns', [])
        for col in no_null_cols:
            if col in df.columns and df[col].isnull().any():
                null_count = df[col].isnull().sum()
                self.errors.append(f"Column {col} has {null_count} null values")
        
        return len(self.errors) == 0
    
    def get_errors(self) -> List[str]:
        """Get validation errors."""
        return self.errors.copy()
"#.to_string()
        },
        metadata: HashMap::new(),
    });

    registry
}

fn create_ai_workflow_prompts() -> InMemoryPromptRegistry {
    let mut registry = InMemoryPromptRegistry::new();

    // 1. Task Analysis Workflow
    registry.add_workflow_prompt(
        "task_analyzer",
        "Analyze a task description for complexity, requirements, and implementation approach",
        "You are an expert software architect and project manager. Analyze tasks for complexity, requirements, potential risks, and suggest implementation approaches.",
        r#"Analyze this task: {{task_description}}

Please provide:
1. Complexity Assessment (Low/Medium/High)
2. Key Requirements and Dependencies
3. Potential Risks and Mitigation Strategies
4. Recommended Implementation Approach
5. Estimated Timeline{{#if technology_stack}} for {{technology_stack}}{{/if}}

Format your response as structured analysis with clear sections."#,
        vec![
            PromptParameter {
                name: "task_description".to_string(),
                description: "Detailed description of the task to analyze".to_string(),
                required: true,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "minLength": 10
                })),
                default: None,
            },
            PromptParameter {
                name: "technology_stack".to_string(),
                description: "Preferred technology stack or platform".to_string(),
                required: false,
                schema: Some(serde_json::json!({
                    "type": "string"
                })),
                default: None,
            },
        ],
    );

    // 2. Code Review Workflow
    registry.add_code_analysis_prompt(
        "code_reviewer",
        "Perform comprehensive code review for {{analysis_type}} analysis",
        "ratchet://code/validator.py",
    );

    // 3. Data Processing Workflow
    registry.add_workflow_prompt(
        "data_processor_designer",
        "Design a data processing pipeline based on requirements",
        "You are a data engineering expert. Design efficient, scalable data processing pipelines.",
        r#"Design a data processing pipeline for: {{data_description}}

Requirements:
- Input: {{input_format}}
- Output: {{output_format}}
- Volume: {{volume_estimate}}
- Processing Type: {{processing_type}}

Please provide:
1. Pipeline Architecture Diagram (text-based)
2. Data Flow Steps
3. Technology Recommendations
4. Performance Considerations
5. Error Handling Strategy
6. Monitoring and Alerting Plan

Consider scalability, maintainability, and cost optimization."#,
        vec![
            PromptParameter {
                name: "data_description".to_string(),
                description: "Description of the data to be processed".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "input_format".to_string(),
                description: "Input data format (CSV, JSON, XML, etc.)".to_string(),
                required: true,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "enum": ["CSV", "JSON", "XML", "Parquet", "Avro", "Other"]
                })),
                default: None,
            },
            PromptParameter {
                name: "output_format".to_string(),
                description: "Desired output format".to_string(),
                required: true,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "enum": ["CSV", "JSON", "XML", "Parquet", "Database", "API", "Other"]
                })),
                default: None,
            },
            PromptParameter {
                name: "volume_estimate".to_string(),
                description: "Expected data volume (e.g., '1GB/day', '1M records/hour')"
                    .to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "processing_type".to_string(),
                description: "Type of processing needed".to_string(),
                required: true,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "enum": ["Batch", "Stream", "Real-time", "ETL", "ELT", "Analytics"]
                })),
                default: None,
            },
        ],
    );

    // 4. Debugging Assistant Workflow
    registry.add_workflow_prompt(
        "debug_assistant",
        "Help debug {{error_type}} errors in {{technology}} applications",
        "You are an expert debugger and troubleshooter. Help identify root causes and provide step-by-step solutions.",
        r#"I'm experiencing a {{error_type}} error in my {{technology}} application.

Error Details:
{{error_message}}

Context:
{{context_description}}

Please help me:
1. Identify the most likely root cause
2. Provide step-by-step debugging approach
3. Suggest immediate fixes
4. Recommend preventive measures
5. Share relevant debugging tools or techniques

Focus on practical, actionable solutions."#,
        vec![
            PromptParameter {
                name: "error_type".to_string(),
                description: "Type of error (runtime, compilation, logic, performance, etc.)".to_string(),
                required: true,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "enum": ["runtime", "compilation", "logic", "performance", "memory", "network", "database", "security"]
                })),
                default: None,
            },
            PromptParameter {
                name: "technology".to_string(),
                description: "Technology stack or programming language".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "error_message".to_string(),
                description: "The actual error message or symptoms".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "context_description".to_string(),
                description: "Additional context about when/how the error occurs".to_string(),
                required: false,
                schema: Some(serde_json::json!({"type": "string"})),
                default: Some(serde_json::Value::String("No additional context provided".to_string())),
            },
        ],
    );

    // 5. API Design Workflow
    registry.add_workflow_prompt(
        "api_designer",
        "Design RESTful APIs for {{domain}} applications",
        "You are an expert API architect. Design clean, efficient, and well-documented APIs following industry best practices.",
        r#"Design a RESTful API for: {{api_purpose}}

Requirements:
- Domain: {{domain}}
- Primary Use Cases: {{use_cases}}
- Expected Load: {{expected_load}}
{{#if authentication_required}}
- Authentication: Required ({{auth_method}})
{{/if}}

Please provide:
1. API Endpoint Design (with HTTP methods)
2. Request/Response Schemas (JSON)
3. Authentication Strategy
4. Error Handling Approach
5. Rate Limiting Recommendations
6. Documentation Structure
7. Testing Strategy

Follow RESTful principles and modern API design patterns."#,
        vec![
            PromptParameter {
                name: "api_purpose".to_string(),
                description: "What the API is designed to accomplish".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "domain".to_string(),
                description: "Business domain (e-commerce, healthcare, finance, etc.)".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "use_cases".to_string(),
                description: "Primary use cases the API should support".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "expected_load".to_string(),
                description: "Expected request volume and performance requirements".to_string(),
                required: true,
                schema: Some(serde_json::json!({"type": "string"})),
                default: None,
            },
            PromptParameter {
                name: "authentication_required".to_string(),
                description: "Whether authentication is required".to_string(),
                required: false,
                schema: Some(serde_json::json!({"type": "boolean"})),
                default: Some(serde_json::Value::Bool(true)),
            },
            PromptParameter {
                name: "auth_method".to_string(),
                description: "Preferred authentication method".to_string(),
                required: false,
                schema: Some(serde_json::json!({
                    "type": "string",
                    "enum": ["OAuth2", "JWT", "API Key", "Basic Auth", "Custom"]
                })),
                default: Some(serde_json::Value::String("JWT".to_string())),
            },
        ],
    );

    // Add categories for organization
    registry.add_category(PromptCategory {
        id: "development".to_string(),
        name: "Software Development".to_string(),
        description: "Prompts for software development workflows".to_string(),
        prompts: vec![
            "task_analyzer".to_string(),
            "code_reviewer".to_string(),
            "debug_assistant".to_string(),
            "api_designer".to_string(),
        ],
    });

    registry.add_category(PromptCategory {
        id: "data".to_string(),
        name: "Data Engineering".to_string(),
        description: "Prompts for data processing and analysis workflows".to_string(),
        prompts: vec!["data_processor_designer".to_string()],
    });

    registry
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 Starting AI Workflow Server with Prompt Registry");
    println!();

    // Create resources and prompts
    let resources = create_ratchet_resources();
    let prompts = create_ai_workflow_prompts();

    println!("📝 AI Workflow Templates Available:");
    println!("  • task_analyzer        - Analyze task complexity and requirements");
    println!("  • code_reviewer         - Comprehensive code review with embedded resources");
    println!("  • data_processor_designer - Design data processing pipelines");
    println!("  • debug_assistant       - Interactive debugging help");
    println!("  • api_designer          - RESTful API design consultation");
    println!();

    // Create server configuration
    let config = McpServerConfig::sse_with_host(3000, "0.0.0.0")
        .with_batch(50)
        .with_metadata("example", serde_json::json!({"type": "ai_workflow"}));

    // Create tools registry
    let mut tools = InMemoryToolRegistry::new();

    // Register a tool that can work with prompts
    let prompt_helper_tool = McpTool::new(
        "list_prompt_categories",
        "List available prompt categories and templates",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        "utility",
    )
    .public();

    tools.register_tool(prompt_helper_tool);

    // Create server state with both registries
    let state = AIWorkflowServerState {
        tools,
        auth: SimpleAuth,
        resources,
        prompts,
    };

    // Create MCP server
    let mcp_server = McpServer::new(config, state);

    // Wrap the server for Axum integration
    let server_wrapper = McpServerWrapper::new(mcp_server);

    // Create Axum app with MCP routes
    let app = axum::Router::new()
        .merge(mcp_routes_with_wrapper())
        .with_state(server_wrapper);

    // Start the server
    println!("🌐 AI Workflow Server running on http://0.0.0.0:3000");
    println!();
    println!("📡 Available Endpoints:");
    println!("  GET  /mcp     - Server capabilities (should show prompts + resources)");
    println!("  POST /mcp     - JSON-RPC requests");
    println!("  GET  /mcp/sse - Server-Sent Events stream");
    println!();
    println!("🧪 Example API Calls:");
    println!();
    println!("1. Check server capabilities (should show both prompts and resources):");
    println!("   curl http://localhost:3000/mcp");
    println!();
    println!("2. List available prompts:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"prompts/list\",\"id\":1}}'");
    println!();
    println!("3. Get a specific prompt template:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"prompts/get\",\"params\":{{\"name\":\"task_analyzer\"}},\"id\":2}}'");
    println!();
    println!("4. Render a prompt with parameters:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"prompts/get\",\"params\":{{\"name\":\"debug_assistant\",\"arguments\":{{\"error_type\":\"runtime\",\"technology\":\"Python\",\"error_message\":\"AttributeError: NoneType object has no attribute split\"}}}},\"id\":3}}'");
    println!();
    println!("5. Get embedded resource prompt (code reviewer):");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"prompts/get\",\"params\":{{\"name\":\"code_reviewer\",\"arguments\":{{\"analysis_type\":\"security\"}}}},\"id\":4}}'");
    println!();
    println!("💡 This demonstrates how prompts can:");
    println!("   • Include parameter substitution ({{variable}})");
    println!("   • Embed external resources (code files, task definitions)");
    println!("   • Provide structured AI interaction patterns");
    println!("   • Support complex workflow templates");
    println!();

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
