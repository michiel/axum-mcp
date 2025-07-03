# axum-mcp

[![Crates.io](https://img.shields.io/crates/v/axum-mcp.svg)](https://crates.io/crates/axum-mcp)
[![Documentation](https://docs.rs/axum-mcp/badge.svg)](https://docs.rs/axum-mcp)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A comprehensive Model Context Protocol (MCP) implementation for Rust with Axum integration, featuring configurable resource registries, AI workflow templates, and multi-transport support.

## Overview

`axum-mcp` is a production-ready MCP server and client library that enables seamless communication between Large Language Models (LLMs) and Rust applications. It provides a trait-based architecture for building custom MCP servers with support for multiple transport protocols.

### Key Features

- 🚀 **Production Ready** - Session management, authentication, monitoring, and error recovery
- 🔌 **Multiple Transports** - stdio, Server-Sent Events (SSE), and StreamableHTTP for Claude Desktop
- 🗂️ **Configurable Resource Registry** - Support for custom URI schemes (`ratchet://`, `layercake://`, etc.)
- 🤖 **AI Workflow Templates** - Prompt registry with parameter substitution and embedded resources
- 🛡️ **Security First** - Built-in authentication, authorization, and rate limiting
- ⚡ **High Performance** - Connection pooling, message batching, and streaming support
- 🎯 **Claude Compatible** - Full support for Claude Desktop's StreamableHTTP transport
- 🧩 **Trait-Based** - Flexible architecture enabling custom tool registries and authentication
- 📊 **Observability** - Comprehensive logging, metrics, and health monitoring

## Architecture

```text
┌─────────────────────┐
│   LLM/AI Agent      │
│  (Claude, GPT-4)    │
│  ┌───────────────┐  │
│  │ MCP Client    │  │
│  └───────┬───────┘  │
└──────────┼──────────┘
           │
    ┌──────┴──────┐
    │   Transport │
    │ (stdio/SSE) │
    └──────┬──────┘
           │
┌──────────▼──────────┐
│  axum-mcp Server    │
│  ┌───────────────┐  │
│  │  Tool Registry│  │
│  └───────┬───────┘  │
└──────────┼──────────┘
           │
    ┌──────┴──────────┐
    │ Your App Logic  │
    │ - Custom Tools  │
    │ - Business Logic│
    │ - Data Access   │
    └─────────────────┘
```

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
axum-mcp = "0.1"
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
serde_json = "1.0"
```

### Basic Server Example

```rust
use axum_mcp::{
    prelude::*,
    axum_integration::{mcp_routes_with_wrapper, McpServerWrapper},
    server::{config::McpServerConfig, service::McpServer},
};

#[derive(Clone)]
struct MyServerState {
    tools: InMemoryToolRegistry,
    auth: MyAuth,
}

#[derive(Clone)]
struct MyAuth;

#[async_trait]
impl McpAuth for MyAuth {
    async fn authenticate(&self, _client: &ClientContext) -> McpResult<SecurityContext> {
        Ok(SecurityContext::system())
    }

    async fn authorize(&self, _context: &SecurityContext, _resource: &str, _action: &str) -> bool {
        true
    }
}

impl McpServerState for MyServerState {
    type ToolRegistry = InMemoryToolRegistry;
    type AuthManager = MyAuth;

    fn tool_registry(&self) -> &Self::ToolRegistry { &self.tools }
    fn auth_manager(&self) -> &Self::AuthManager { &self.auth }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = McpServerConfig::default();
    
    // Create tools registry
    let mut tools = InMemoryToolRegistry::new();
    let echo_tool = McpTool::new(
        "echo",
        "Echo back a message",
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Message to echo"}
            },
            "required": ["message"]
        }),
        "utility"
    ).public();
    tools.register_tool(echo_tool);
    
    let state = MyServerState {
        tools,
        auth: MyAuth,
    };
    
    let server = McpServer::new(config, state);
    let wrapper = McpServerWrapper::new(server);
    
    let app = axum::Router::new()
        .merge(mcp_routes_with_wrapper())
        .with_state(wrapper);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("MCP server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Claude Desktop Integration

To integrate with Claude Desktop, add this configuration to Claude's settings:

```json
{
  "mcpServers": {
    "my-rust-server": {
      "command": "curl",
      "args": [
        "-X", "POST",
        "http://localhost:3000/mcp",
        "-H", "Content-Type: application/json",
        "-d", "@-"
      ],
      "transport": "streamable-http"
    }
  }
}
```

## Core Concepts

### Tool Registry

The `ToolRegistry` trait defines how your application exposes tools to MCP clients:

```rust
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    async fn list_tools(&self, context: &SecurityContext) -> McpResult<Vec<Tool>>;
    async fn execute_tool(&self, name: &str, context: ToolExecutionContext) -> McpResult<ToolsCallResult>;
}
```

### Server State

The `McpServerState` trait connects your tool registry, authentication, and optional registries:

```rust
pub trait McpServerState: Send + Sync + Clone + 'static {
    type ToolRegistry: ToolRegistry;
    type AuthManager: McpAuth;
    
    fn tool_registry(&self) -> &Self::ToolRegistry;
    fn auth_manager(&self) -> &Self::AuthManager;
    
    // Optional: Add resource registry for custom URI schemes
    fn resource_registry(&self) -> Option<&dyn ResourceRegistry> { None }
    
    // Optional: Add prompt registry for AI workflow templates  
    fn prompt_registry(&self) -> Option<&dyn PromptRegistry> { None }
}
```

### Authentication

Implement custom authentication with the `McpAuth` trait:

```rust
#[async_trait]
pub trait McpAuth: Send + Sync {
    async fn authenticate(&self, context: &ClientContext) -> McpResult<SecurityContext>;
    async fn authorize(&self, context: &SecurityContext, resource: &str, action: &str) -> bool;
}
```

## Transport Types

### Standard I/O Transport

For local processes and command-line tools:

```rust
use axum_mcp::transport::{StdioTransport, McpTransport};

let transport = StdioTransport::new();
// Use with stdio-based MCP clients
```

### Server-Sent Events (SSE)

For web-based real-time communication:

```rust
// SSE endpoints are automatically included in mcp_routes()
let app = Router::new()
    .merge(mcp_routes())
    .with_state(state);
```

### StreamableHTTP

For Claude Desktop compatibility:

```rust
// StreamableHTTP is the default transport in mcp_routes()
// Supports both request/response and streaming modes
```

## Advanced Features

### Custom Authentication

```rust
#[derive(Clone)]
struct ApiKeyAuth {
    valid_keys: HashSet<String>,
}

#[async_trait]
impl McpAuth for ApiKeyAuth {
    async fn authenticate(&self, context: &ClientContext) -> McpResult<SecurityContext> {
        let api_key = context.headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| McpError::unauthorized("Missing API key"))?;
            
        if self.valid_keys.contains(api_key) {
            Ok(SecurityContext::authenticated(api_key.to_string()))
        } else {
            Err(McpError::unauthorized("Invalid API key"))
        }
    }
    
    async fn authorize(&self, _context: &SecurityContext, _resource: &str, _action: &str) -> bool {
        true // Implement your authorization logic
    }
}
```

### Progress Reporting

For long-running operations:

```rust
async fn execute_tool(&self, name: &str, context: ToolExecutionContext) -> McpResult<ToolsCallResult> {
    match name {
        "long_task" => {
            let progress = context.progress_reporter();
            
            progress.report(0.0, "Starting task...").await;
            // Do work...
            progress.report(0.5, "Half way done...").await;
            // More work...
            progress.report(1.0, "Complete!").await;
            
            Ok(ToolsCallResult::text("Task completed"))
        }
        _ => Err(McpError::method_not_found(format!("Unknown tool: {}", name)))
    }
}
```

### Rate Limiting

```rust
use axum_mcp::security::RateLimiter;

let rate_limiter = RateLimiter::new(100, Duration::from_secs(60)); // 100 requests per minute
```

## Resource Registry

The resource registry enables custom URI schemes for project-specific resources:

### Basic Resource Registry

```rust
use axum_mcp::server::resource::{UriSchemeConfig, InMemoryResourceRegistry, Resource, ResourceContent};

// Configure your project's URI scheme
let scheme = UriSchemeConfig::new("myproject", "My Project Resources")
    .with_types(vec!["task".to_string(), "model".to_string()]);

let mut registry = InMemoryResourceRegistry::new(scheme);

// Add resources with your custom URIs
registry.add_resource(Resource {
    uri: "myproject://tasks/data-processor".to_string(),
    name: "Data Processing Task".to_string(),
    description: Some("Processes CSV data files".to_string()),
    mime_type: Some("application/json".to_string()),
    content: ResourceContent::Text {
        text: r#"{"name": "data-processor", "type": "etl"}"#.to_string()
    },
    metadata: std::collections::HashMap::new(),
});
```

### Multi-Scheme Support

Support multiple URI schemes simultaneously:

```rust
use axum_mcp::server::resource::MultiSchemeResourceRegistry;

let mut multi_registry = MultiSchemeResourceRegistry::new();

// Register different schemes for different projects
let ratchet_registry = create_ratchet_registry();  // ratchet:// URIs
let layercake_registry = create_layercake_registry();  // layercake:// URIs

multi_registry.register_scheme(Box::new(ratchet_registry));
multi_registry.register_scheme(Box::new(layercake_registry));

// Now supports both ratchet://tasks/... and layercake://models/...
```

### Resource Templates

Define templates for dynamic resource discovery:

```rust
registry.add_template(ResourceTemplate {
    uri_template: "myproject://tasks/{task_id}".to_string(),
    name: "Task Definition".to_string(),
    description: Some("Task configuration and metadata".to_string()),
    mime_type: Some("application/json".to_string()),
    metadata: {
        let mut meta = HashMap::new();
        meta.insert("parameters".to_string(), serde_json::json!({
            "task_id": "The unique identifier for the task"
        }));
        meta
    },
});
```

## Prompt Registry

Create reusable AI workflow templates with parameter substitution:

### Basic Prompt Registry

```rust
use axum_mcp::server::prompt::{InMemoryPromptRegistry, PromptParameter};

let mut prompts = InMemoryPromptRegistry::new();

// Add a workflow prompt with parameters
prompts.add_workflow_prompt(
    "code_analyzer",
    "Analyze code for {{analysis_type}} issues",
    "You are an expert code reviewer.",
    "Please analyze this code for {{analysis_type}} issues: {{code_content}}",
    vec![
        PromptParameter {
            name: "analysis_type".to_string(),
            description: "Type of analysis (security, performance, style)".to_string(),
            required: true,
            schema: Some(serde_json::json!({
                "type": "string",
                "enum": ["security", "performance", "style"]
            })),
            default: None,
        },
        PromptParameter {
            name: "code_content".to_string(),
            description: "The code to analyze".to_string(),
            required: true,
            schema: Some(serde_json::json!({"type": "string"})),
            default: None,
        },
    ],
);
```

### Embedded Resources in Prompts

Link prompts to external resources:

```rust
// Create a prompt that embeds a code file
prompts.add_code_analysis_prompt(
    "security_reviewer",
    "Review code for security vulnerabilities",
    "myproject://code/auth_handler.py"  // Embedded resource URI
);

// When rendered, this automatically includes the code file content
```

### Using Prompts with Parameters

```rust
let request = GetPromptRequest {
    name: "code_analyzer".to_string(),
    arguments: Some({
        let mut args = HashMap::new();
        args.insert("analysis_type".to_string(), 
                   serde_json::Value::String("security".to_string()));
        args.insert("code_content".to_string(),
                   serde_json::Value::String("def authenticate(user): return True".to_string()));
        args
    }),
};

let result = prompts.get_prompt_with_args(request, &context).await?;
// Returns rendered prompt with parameters substituted
```

### AI Workflow Categories

Organize prompts by domain:

```rust
prompts.add_category(PromptCategory {
    id: "development".to_string(),
    name: "Software Development".to_string(), 
    description: "Prompts for software development workflows".to_string(),
    prompts: vec![
        "code_analyzer".to_string(),
        "debug_assistant".to_string(),
        "api_designer".to_string(),
    ],
});
```

## Configuration

Configure your MCP server with various options:

```rust
let config = McpServerConfig::sse_with_host(3000, "0.0.0.0")
    .with_batch(50)  // Enable batch operations with max 50 items
    .with_metadata("version", serde_json::json!("1.0.0"))
    .with_timeout(std::time::Duration::from_secs(30));
```

### Transport Configuration

Choose your transport type:

```rust
// Server-Sent Events (default)
let config = McpServerConfig::sse_with_host(3000, "0.0.0.0");

// StreamableHTTP for Claude Desktop
let config = McpServerConfig::streamable_http_with_host(3000, "0.0.0.0");

// stdio for command-line tools
let config = McpServerConfig::stdio();
```

## Error Handling

Comprehensive error handling with typed errors:

```rust
use axum_mcp::{McpError, McpResult};

fn my_tool_function() -> McpResult<String> {
    // Tool not found
    Err(McpError::ToolNotFound {
        name: "invalid_tool".to_string()
    })
    
    // Tool execution error
    Err(McpError::ToolExecution {
        tool: "my_tool".to_string(),
        message: "Invalid parameters".to_string()
    })
    
    // Resource errors
    Err(McpError::ResourceNotFound {
        uri: "myproject://invalid/resource".to_string()
    })
    
    // Authentication errors
    Err(McpError::Authentication {
        message: "Invalid credentials".to_string()
    })
}
```

## Project Integration Examples

### Ratchet Integration

```rust
// Ratchet uses ratchet:// URI scheme
let ratchet_scheme = UriSchemeConfig::new("ratchet", "Ratchet task management")
    .with_types(vec!["task".to_string(), "execution".to_string()]);

// Resources: ratchet://tasks/web-scraper, ratchet://executions/run-123
// Prompts: Task analysis, execution debugging, workflow optimization
```

### Layercake Integration

```rust
// Layercake uses layercake:// URI scheme  
let layercake_scheme = UriSchemeConfig::new("layercake", "Layercake ML platform")
    .with_types(vec!["model".to_string(), "dataset".to_string()]);

// Resources: layercake://models/sentiment-v2, layercake://datasets/training-data
// Prompts: Model evaluation, hyperparameter tuning, data analysis
```

## Features

Enable specific features in your `Cargo.toml`:

```toml
[dependencies]
axum-mcp = { version = "0.1", features = ["server", "client", "transport-sse"] }
```

Available features:
- `server` - MCP server implementation (default)
- `client` - MCP client implementation
- `transport-stdio` - Standard I/O transport (default)
- `transport-sse` - Server-Sent Events transport (default)
- `transport-streamable-http` - StreamableHTTP transport for Claude Desktop (default)

## Examples

The `examples/` directory contains comprehensive examples:

- **[`minimal_server.rs`](examples/minimal_server.rs)** - Basic MCP server setup with tool registry
- **[`resource_registry_example.rs`](examples/resource_registry_example.rs)** - Custom URI schemes and resource management  
- **[`prompt_registry_example.rs`](examples/prompt_registry_example.rs)** - AI workflow templates and prompt management

### Running Examples

```bash
# Basic MCP server
cargo run --example minimal_server

# Resource registry with custom URI schemes
cargo run --example resource_registry_example

# AI workflow templates with prompt registry  
cargo run --example prompt_registry_example
```

Each example includes detailed API usage instructions and curl commands for testing.

## Testing

Run the test suite:

```bash
cargo test
```

Run examples:

```bash
cargo run --example minimal_server
```

## Contributing

Contributions are welcome! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

This crate was extracted from the [Ratchet](https://github.com/ratchet-org/ratchet) project and represents a comprehensive, production-ready MCP implementation for the Rust ecosystem.