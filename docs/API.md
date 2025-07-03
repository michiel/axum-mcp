# axum-mcp API Documentation

This document provides comprehensive API documentation for the axum-mcp crate, including all supported MCP protocol methods and custom extensions.

## Table of Contents

- [Core MCP Protocol](#core-mcp-protocol)
- [Tool Registry API](#tool-registry-api)
- [Resource Registry API](#resource-registry-api)
- [Prompt Registry API](#prompt-registry-api)
- [Authentication & Security](#authentication--security)
- [Error Handling](#error-handling)
- [Configuration](#configuration)

## Core MCP Protocol

axum-mcp implements the complete MCP (Model Context Protocol) specification with JSON-RPC 2.0.

### Standard Methods

| Method | Description | Parameters | Response |
|--------|-------------|------------|----------|
| `initialize` | Initialize the MCP session | `InitializeParams` | `InitializeResult` |
| `tools/list` | List available tools | None | `ToolsListResult` |
| `tools/call` | Execute a tool | `ToolsCallParams` | `ToolsCallResult` |
| `resources/list` | List resource templates | None | `ResourceTemplate[]` |
| `resources/read` | Read a specific resource | `ResourceReadParams` | `Resource` |
| `prompts/list` | List available prompts | None | `Prompt[]` |
| `prompts/get` | Get a prompt with parameters | `GetPromptRequest` | `GetPromptResult` |

### Message Format

All requests follow JSON-RPC 2.0 format:

```json
{
  "jsonrpc": "2.0",
  "method": "method_name",
  "params": {
    // method-specific parameters
  },
  "id": 1
}
```

Responses:

```json
{
  "jsonrpc": "2.0",
  "result": {
    // method-specific result
  },
  "id": 1
}
```

## Tool Registry API

### List Tools

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "tools/list",
  "id": 1
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "tools": [
      {
        "name": "echo",
        "description": "Echo back a message",
        "input_schema": {
          "type": "object",
          "properties": {
            "message": {
              "type": "string",
              "description": "Message to echo"
            }
          },
          "required": ["message"]
        },
        "metadata": {}
      }
    ]
  },
  "id": 1
}
```

### Execute Tool

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "echo",
    "arguments": {
      "message": "Hello, World!"
    }
  },
  "id": 2
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Echo: Hello, World!"
      }
    ],
    "is_error": false,
    "metadata": {}
  },
  "id": 2
}
```

## Resource Registry API

The resource registry provides access to project resources through custom URI schemes.

### List Resource Templates

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "resources/templates/list",
  "id": 3
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "resourceTemplates": [
      {
        "uri_template": "ratchet://tasks/{task_id}",
        "name": "Task Definition",
        "description": "Ratchet task configuration and metadata",
        "mime_type": "application/json",
        "metadata": {
          "parameters": {
            "task_id": "The unique identifier for the task"
          }
        }
      }
    ]
  },
  "id": 3
}
```

### Read Resource

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "resources/read",
  "params": {
    "uri": "ratchet://tasks/web-scraper"
  },
  "id": 4
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "contents": [
      {
        "uri": "ratchet://tasks/web-scraper",
        "mime_type": "application/json",
        "text": "{\"name\": \"web-scraper\", \"description\": \"Scrape web content\", \"schedule\": \"0 */6 * * *\"}"
      }
    ]
  },
  "id": 4
}
```

### Subscribe to Resource Changes

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "resources/subscribe",
  "params": {
    "uri": "ratchet://tasks/*"
  },
  "id": 5
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "subscription_id": "sub_12345"
  },
  "id": 5
}
```

### Supported URI Schemes

| Scheme | Purpose | Example URIs |
|--------|---------|--------------|
| `ratchet://` | Ratchet task management | `ratchet://tasks/web-scraper`, `ratchet://executions/run-123` |
| `layercake://` | Layercake ML platform | `layercake://models/sentiment-v2`, `layercake://datasets/training` |
| Custom schemes | Project-specific resources | `myproject://data/config.json` |

## Prompt Registry API

The prompt registry provides reusable AI workflow templates with parameter substitution.

### List Prompts

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "prompts/list",
  "id": 6
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "prompts": [
      {
        "name": "task_analyzer",
        "description": "Analyze a task for complexity and requirements",
        "version": "1.0.0",
        "parameters": [
          {
            "name": "task_description",
            "description": "Description of the task to analyze",
            "required": true,
            "schema": {
              "type": "string",
              "minLength": 1
            },
            "default": null
          }
        ],
        "metadata": {
          "type": "workflow"
        }
      }
    ]
  },
  "id": 6
}
```

### Get Prompt Template

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "prompts/get",
  "params": {
    "name": "task_analyzer"
  },
  "id": 7
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "task_analyzer",
    "description": "Analyze a task for complexity and requirements",
    "messages": [
      {
        "role": "system",
        "content": {
          "type": "text",
          "text": "You are an expert task analyst. Analyze tasks for complexity and requirements."
        }
      },
      {
        "role": "user", 
        "content": {
          "type": "text",
          "text": "Analyze this task: {{task_description}}. Provide complexity rating and requirements."
        }
      }
    ]
  },
  "id": 7
}
```

### Get Prompt with Parameters

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "prompts/get",
  "params": {
    "name": "task_analyzer",
    "arguments": {
      "task_description": "Build a real-time data processing pipeline for IoT sensors"
    }
  },
  "id": 8
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "task_analyzer",
    "description": "Analyze a task for complexity and requirements",
    "messages": [
      {
        "role": "system",
        "content": {
          "type": "text",
          "text": "You are an expert task analyst. Analyze tasks for complexity and requirements."
        }
      },
      {
        "role": "user",
        "content": {
          "type": "text", 
          "text": "Analyze this task: Build a real-time data processing pipeline for IoT sensors. Provide complexity rating and requirements."
        }
      }
    ]
  },
  "id": 8
}
```

### Embedded Resource Prompts

Some prompts can embed external resources:

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "prompts/get",
  "params": {
    "name": "code_reviewer",
    "arguments": {
      "analysis_type": "security"
    }
  },
  "id": 9
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "code_reviewer",
    "description": "Review code for security issues",
    "messages": [
      {
        "role": "system",
        "content": {
          "type": "text",
          "text": "You are an expert code reviewer. Analyze code for security vulnerabilities."
        }
      },
      {
        "role": "user",
        "content": {
          "type": "embedded_resource",
          "resource": {
            "uri": "ratchet://code/validator.py",
            "mime_type": "text/x-python",
            "annotation": {
              "description": "Source code to analyze",
              "role": "primary_input"
            }
          },
          "text": "Please perform a security analysis of this code. Focus on potential vulnerabilities."
        }
      }
    ]
  },
  "id": 9
}
```

## Authentication & Security

### API Key Authentication

```http
POST /mcp
Authorization: Bearer your-api-key-here
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "tools/list",
  "id": 1
}
```

### JWT Authentication

```http
POST /mcp
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "tools/list", 
  "id": 1
}
```

### Client Context

The server automatically extracts client information from request headers:

```rust
struct ClientContext {
    user_agent: String,
    client_id: Option<String>,
    session_id: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
}
```

## Error Handling

### Standard JSON-RPC Errors

| Code | Message | Description |
|------|---------|-------------|
| -32600 | Invalid Request | Invalid JSON-RPC request |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Server internal error |

### MCP-Specific Errors

| Code | Type | Description |
|------|------|-------------|
| -32000 | Tool Not Found | Requested tool doesn't exist |
| -32001 | Tool Execution Error | Tool execution failed |
| -32002 | Resource Not Found | Requested resource doesn't exist |
| -32003 | Authentication Required | Missing or invalid authentication |
| -32004 | Authorization Failed | Insufficient permissions |
| -32005 | Rate Limit Exceeded | Too many requests |

Example error response:
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Tool 'invalid_tool' not found",
    "data": {
      "tool_name": "invalid_tool",
      "available_tools": ["echo", "calculator"]
    }
  },
  "id": 1
}
```

## Configuration

### Server Configuration

```rust
let config = McpServerConfig::sse_with_host(3000, "0.0.0.0")
    .with_batch(50)  // Max batch size
    .with_timeout(Duration::from_secs(30))  // Request timeout
    .with_metadata("version", json!("1.0.0"));  // Server metadata
```

### Transport Options

| Transport | Use Case | Configuration |
|-----------|----------|---------------|
| **SSE** | Web applications, real-time updates | `McpServerConfig::sse_with_host(port, host)` |
| **StreamableHTTP** | Claude Desktop compatibility | `McpServerConfig::streamable_http_with_host(port, host)` |
| **stdio** | Command-line tools, local processes | `McpServerConfig::stdio()` |

### Feature Flags

Enable specific features in your `Cargo.toml`:

```toml
[dependencies]
axum-mcp = { version = "0.1", features = [
    "server",                    # MCP server implementation
    "client",                    # MCP client implementation  
    "transport-stdio",           # stdio transport
    "transport-sse",             # Server-Sent Events transport
    "transport-streamable-http", # StreamableHTTP transport for Claude Desktop
    "handlers"                   # Axum HTTP handlers
]}
```

## Rate Limiting

Configure rate limiting per client:

```rust
let rate_limiter = RateLimiter::new(
    100,                          // Max requests
    Duration::from_secs(60)       // Time window (1 minute)
);
```

Rate limit headers in responses:
```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```

## Health Monitoring

### Health Check Endpoint

```http
GET /mcp?transport=health
```

Response:
```json
{
  "status": "healthy",
  "message": "Server is running normally",
  "uptime_seconds": 3600,
  "active_connections": 5,
  "transport_health": {
    "status": "healthy",
    "latency_ms": 2.5
  }
}
```

### Server Information

```http
GET /mcp
```

Response:
```json
{
  "name": "My MCP Server",
  "protocol_versions": ["1.0.0"],
  "transports": ["sse", "streamable_http"],
  "capabilities": ["tools", "resources", "prompts"],
  "session_support": true
}
```

## Batch Operations

Execute multiple requests in a single call:

```http
POST /mcp
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "batch",
  "params": {
    "requests": [
      {
        "method": "tools/list",
        "params": {},
        "id": 1
      },
      {
        "method": "resources/templates/list", 
        "params": {},
        "id": 2
      }
    ]
  },
  "id": "batch_1"
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "responses": [
      {
        "jsonrpc": "2.0",
        "result": { "tools": [...] },
        "id": 1
      },
      {
        "jsonrpc": "2.0", 
        "result": { "resourceTemplates": [...] },
        "id": 2
      }
    ]
  },
  "id": "batch_1"
}
```

## Progress Reporting

For long-running operations, servers can send progress updates via SSE:

```
event: progress
data: {"operation_id": "op_123", "progress": 0.5, "message": "Processing..."}

event: progress  
data: {"operation_id": "op_123", "progress": 1.0, "message": "Complete!"}
```

## Session Management

### StreamableHTTP Sessions

For Claude Desktop compatibility:

```http
GET /mcp/sse?session_id=sess_123&last_event_id=event_456
```

Sessions support:
- **Resumable connections** - Reconnect with last event ID
- **Event history** - Replay missed events
- **Automatic cleanup** - Sessions expire after inactivity
- **Health monitoring** - Connection status tracking