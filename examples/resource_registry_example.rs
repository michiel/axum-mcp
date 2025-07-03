//! Resource registry example with configurable URI schemes
//!
//! This example demonstrates how to create MCP servers with different URI schemes:
//! - ratchet:// for Ratchet project resources
//! - layercake:// for Layercake project resources
//! - custom:// for custom project resources
//!
//! Run with: cargo run --example resource_registry_example

use axum_mcp::{
    prelude::*,
    axum_integration::{mcp_routes_with_wrapper, McpServerWrapper},
    server::{
        config::McpServerConfig, service::McpServer,
        resource::{UriSchemeConfig, InMemoryResourceRegistry, Resource, ResourceContent, ResourceTemplate}
    },
    protocol::ServerInfo,
    ResourceRegistry,
};
use std::collections::HashMap;
use tokio::net::TcpListener;

// Define server state with resource registry support
#[derive(Clone)]
struct ExampleServerState {
    tools: InMemoryToolRegistry,
    auth: SimpleAuth,
    resources: InMemoryResourceRegistry,
}

// Simple authentication that allows everything
#[derive(Clone)]
struct SimpleAuth;

#[async_trait]
impl McpAuth for SimpleAuth {
    async fn authenticate(&self, _client_info: &ClientContext) -> McpResult<SecurityContext> {
        // For this example, all clients get full access
        Ok(SecurityContext::system())
    }

    async fn authorize(&self, _context: &SecurityContext, _resource: &str, _action: &str) -> bool {
        // Allow all operations
        true
    }
}

impl McpServerState for ExampleServerState {
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

    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            name: "Resource Registry Example Server".to_string(),
            version: "1.0.0".to_string(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

fn create_ratchet_resources() -> InMemoryResourceRegistry {
    // Configure Ratchet URI scheme
    let ratchet_scheme = UriSchemeConfig::new("ratchet", "Ratchet task management resources")
        .with_types(vec![
            "task".to_string(),
            "execution".to_string(), 
            "schedule".to_string(),
            "log".to_string()
        ]);
    
    let mut registry = InMemoryResourceRegistry::new(ratchet_scheme);
    
    // Add some example Ratchet resources
    registry.add_resource(Resource {
        uri: "ratchet://tasks/web-scraper".to_string(),
        name: "Web Scraper Task".to_string(),
        description: Some("A task that scrapes web content".to_string()),
        mime_type: Some("application/json".to_string()),
        content: ResourceContent::Text {
            text: r#"{
  "name": "web-scraper",
  "description": "Scrape product information from e-commerce sites",
  "schedule": "0 */6 * * *",
  "config": {
    "urls": ["https://example.com/products"],
    "selectors": {
      "title": ".product-title",
      "price": ".price"
    }
  }
}"#.to_string()
        },
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("category".to_string(), serde_json::Value::String("automation".to_string()));
            meta.insert("priority".to_string(), serde_json::Value::String("high".to_string()));
            meta
        },
    });

    registry.add_resource(Resource {
        uri: "ratchet://executions/web-scraper-20250103-140000".to_string(),
        name: "Web Scraper Execution Log".to_string(),
        description: Some("Execution log from January 3rd, 2025".to_string()),
        mime_type: Some("text/plain".to_string()),
        content: ResourceContent::Text {
            text: r#"2025-01-03 14:00:00 [INFO] Starting web scraper task
2025-01-03 14:00:01 [INFO] Connecting to https://example.com/products
2025-01-03 14:00:02 [INFO] Found 25 products
2025-01-03 14:00:03 [INFO] Processing product data
2025-01-03 14:00:05 [INFO] Task completed successfully
2025-01-03 14:00:05 [INFO] Results stored in database"#.to_string()
        },
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("execution_id".to_string(), serde_json::Value::String("web-scraper-20250103-140000".to_string()));
            meta.insert("status".to_string(), serde_json::Value::String("success".to_string()));
            meta.insert("duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(5000)));
            meta
        },
    });

    // Add resource templates
    registry.add_template(ResourceTemplate {
        uri_template: "ratchet://tasks/{task_id}".to_string(),
        name: "Task Definition".to_string(),
        description: Some("Ratchet task configuration and metadata".to_string()),
        mime_type: Some("application/json".to_string()),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("parameters".to_string(), serde_json::json!({
                "task_id": "The unique identifier for the task"
            }));
            meta
        },
    });

    registry.add_template(ResourceTemplate {
        uri_template: "ratchet://executions/{execution_id}".to_string(),
        name: "Execution Log".to_string(),
        description: Some("Logs and results from a task execution".to_string()),
        mime_type: Some("text/plain".to_string()),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("parameters".to_string(), serde_json::json!({
                "execution_id": "The unique identifier for the execution (format: {task_id}-{timestamp})"
            }));
            meta
        },
    });

    registry
}

fn create_layercake_resources() -> InMemoryResourceRegistry {
    // Configure Layercake URI scheme
    let layercake_scheme = UriSchemeConfig::new("layercake", "Layercake ML model resources")
        .with_types(vec![
            "model".to_string(),
            "dataset".to_string(),
            "experiment".to_string(),
            "artifact".to_string()
        ]);
    
    let mut registry = InMemoryResourceRegistry::new(layercake_scheme);
    
    // Add example Layercake resources
    registry.add_resource(Resource {
        uri: "layercake://models/sentiment-classifier-v2".to_string(),
        name: "Sentiment Classifier Model v2".to_string(),
        description: Some("Production sentiment analysis model".to_string()),
        mime_type: Some("application/json".to_string()),
        content: ResourceContent::Text {
            text: r#"{
  "model_id": "sentiment-classifier-v2",
  "version": "2.1.0",
  "architecture": "transformer",
  "base_model": "bert-base-uncased",
  "accuracy": 0.94,
  "training_data": "layercake://datasets/sentiment-train-v2",
  "deployment": {
    "status": "active",
    "endpoint": "https://api.layercake.ai/models/sentiment-classifier-v2",
    "scaling": "auto"
  }
}"#.to_string()
        },
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("model_type".to_string(), serde_json::Value::String("classification".to_string()));
            meta.insert("framework".to_string(), serde_json::Value::String("pytorch".to_string()));
            meta.insert("size_mb".to_string(), serde_json::Value::Number(serde_json::Number::from(428)));
            meta
        },
    });

    registry.add_resource(Resource {
        uri: "layercake://datasets/sentiment-train-v2".to_string(),
        name: "Sentiment Training Dataset v2".to_string(),
        description: Some("Curated training dataset for sentiment analysis".to_string()),
        mime_type: Some("application/json".to_string()),
        content: ResourceContent::Text {
            text: r#"{
  "dataset_id": "sentiment-train-v2",
  "version": "2.0.0",
  "size": 50000,
  "split": {
    "train": 40000,
    "validation": 5000,
    "test": 5000
  },
  "labels": ["positive", "negative", "neutral"],
  "source": "multi-domain social media and reviews",
  "quality_score": 0.96
}"#.to_string()
        },
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("dataset_type".to_string(), serde_json::Value::String("text_classification".to_string()));
            meta.insert("language".to_string(), serde_json::Value::String("en".to_string()));
            meta.insert("created_date".to_string(), serde_json::Value::String("2024-12-15".to_string()));
            meta
        },
    });

    // Add resource templates
    registry.add_template(ResourceTemplate {
        uri_template: "layercake://models/{model_id}".to_string(),
        name: "ML Model".to_string(),
        description: Some("Machine learning model configuration and metadata".to_string()),
        mime_type: Some("application/json".to_string()),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("parameters".to_string(), serde_json::json!({
                "model_id": "The unique identifier for the model"
            }));
            meta
        },
    });

    registry.add_template(ResourceTemplate {
        uri_template: "layercake://datasets/{dataset_id}".to_string(),
        name: "Training Dataset".to_string(),
        description: Some("Dataset used for model training and evaluation".to_string()),
        mime_type: Some("application/json".to_string()),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("parameters".to_string(), serde_json::json!({
                "dataset_id": "The unique identifier for the dataset"
            }));
            meta
        },
    });

    registry
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 Starting Resource Registry Example Server");
    println!();
    
    // Demonstrate different URI schemes
    println!("📋 Supported URI Schemes:");
    
    // Create registries for different projects
    let ratchet_resources = create_ratchet_resources();
    let layercake_resources = create_layercake_resources();
    
    println!("  • ratchet://    - Ratchet task management ({})", 
        ratchet_resources.uri_scheme().description);
    println!("  • layercake://  - Layercake ML platform ({})", 
        layercake_resources.uri_scheme().description);
    println!();

    // For this example, we'll demonstrate with the Ratchet scheme
    // In a real implementation, you might use MultiSchemeResourceRegistry
    // to support multiple schemes simultaneously
    
    // Create server configuration
    let config = McpServerConfig::sse_with_host(3000, "0.0.0.0")
        .with_batch(50)
        .with_metadata("example", serde_json::json!({"type": "resource_registry"}));

    // Create tools registry with an example tool
    let mut tools = InMemoryToolRegistry::new();
    
    // Register a tool that can work with resources
    let resource_info_tool = McpTool::new(
        "get_resource_info",
        "Get information about a resource by URI",
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": {
                    "type": "string",
                    "description": "Resource URI (e.g., 'ratchet://tasks/my-task')"
                }
            },
            "required": ["uri"]
        }),
        "utility"
    ).public();
    
    tools.register_tool(resource_info_tool);

    // Create server state with resource registry
    let state = ExampleServerState {
        tools,
        auth: SimpleAuth,
        resources: ratchet_resources,
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
    println!("🌐 MCP Server running on http://0.0.0.0:3000");
    println!();
    println!("📡 Available Endpoints:");
    println!("  GET  /mcp     - Server information and capabilities");
    println!("  POST /mcp     - JSON-RPC requests");
    println!("  GET  /mcp/sse - Server-Sent Events stream");
    println!();
    println!("🧪 Example API Calls:");
    println!();
    println!("1. Check server capabilities (should show resources support):");
    println!("   curl http://localhost:3000/mcp");
    println!();
    println!("2. List available resource templates:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"resources/templates/list\",\"id\":1}}'");
    println!();
    println!("3. Get a specific Ratchet task resource:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"resources/read\",\"params\":{{\"uri\":\"ratchet://tasks/web-scraper\"}},\"id\":2}}'");
    println!();
    println!("4. Get a Ratchet execution log:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"resources/read\",\"params\":{{\"uri\":\"ratchet://executions/web-scraper-20250103-140000\"}},\"id\":3}}'");
    println!();
    println!("5. Use the resource info tool:");
    println!("   curl -X POST http://localhost:3000/mcp \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{{\"name\":\"get_resource_info\",\"arguments\":{{\"uri\":\"ratchet://tasks/web-scraper\"}}}},\"id\":4}}'");
    println!();
    println!("💡 Note: This example demonstrates the 'ratchet://' URI scheme.");
    println!("   In a real deployment, you would configure the appropriate scheme for your project.");
    println!("   Layercake would use 'layercake://' URIs, custom projects would define their own schemes.");
    println!();

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}