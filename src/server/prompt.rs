//! Prompt registry for MCP prompts and AI workflow templates
//!
//! This module provides a flexible prompt registry system that supports
//! reusable AI interaction patterns, workflow templates, and dynamic prompt generation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

use crate::{
    error::{McpError, McpResult},
    security::SecurityContext,
};

/// Prompt content with support for text and embedded resources
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    /// Plain text content
    Text { text: String },
    /// Content with embedded resources
    EmbeddedResource { 
        resource: EmbeddedResource,
        text: Option<String>,
    },
}

/// Embedded resource reference within a prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedResource {
    /// Resource URI (e.g., "ratchet://tasks/web-scraper")
    pub uri: String,
    /// Optional MIME type override
    pub mime_type: Option<String>,
    /// Resource annotation for the AI model
    pub annotation: Option<ResourceAnnotation>,
}

/// Annotation providing context about an embedded resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAnnotation {
    /// Human-readable description of the resource
    pub description: String,
    /// Role of this resource in the prompt (e.g., "example", "template", "context")
    pub role: String,
}

/// Message role in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Individual message in a prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role of this message
    pub role: MessageRole,
    /// Message content
    pub content: PromptContent,
}

/// Prompt template parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptParameter {
    /// Parameter name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Whether this parameter is required
    pub required: bool,
    /// JSON schema for parameter validation
    pub schema: Option<serde_json::Value>,
    /// Default value if not provided
    pub default: Option<serde_json::Value>,
}

/// Complete prompt template definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique prompt identifier
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Prompt version for compatibility tracking
    pub version: String,
    /// Template parameters
    pub parameters: Vec<PromptParameter>,
    /// Template messages
    pub messages: Vec<PromptMessage>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Request to get a specific prompt with parameter values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptRequest {
    /// Name of the prompt to retrieve
    pub name: String,
    /// Parameter values for template substitution
    pub arguments: Option<HashMap<String, serde_json::Value>>,
}

/// Rendered prompt result with parameter substitution applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptResult {
    /// Original prompt name
    pub name: String,
    /// Rendered messages with parameters substituted
    pub messages: Vec<PromptMessage>,
    /// Description with any parameter substitutions
    pub description: String,
}

/// Prompt category for organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCategory {
    /// Category identifier
    pub id: String,
    /// Human-readable category name
    pub name: String,
    /// Category description
    pub description: String,
    /// List of prompt names in this category
    pub prompts: Vec<String>,
}

/// Template engine interface for parameter substitution
pub trait TemplateEngine: Send + Sync {
    /// Substitute parameters in a text template
    fn substitute(&self, template: &str, params: &HashMap<String, serde_json::Value>) -> McpResult<String>;
    
    /// Validate that all required parameters are provided
    fn validate_parameters(&self, template: &str, params: &HashMap<String, serde_json::Value>, required: &[String]) -> McpResult<()>;
}

/// Simple template engine using basic string replacement
#[derive(Debug, Clone)]
pub struct SimpleTemplateEngine;

impl TemplateEngine for SimpleTemplateEngine {
    fn substitute(&self, template: &str, params: &HashMap<String, serde_json::Value>) -> McpResult<String> {
        let mut result = template.to_string();
        
        for (key, value) in params {
            let placeholder = format!("{{{{{}}}}}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "".to_string(),
                _ => serde_json::to_string(value).unwrap_or_default(),
            };
            result = result.replace(&placeholder, &replacement);
        }
        
        Ok(result)
    }
    
    fn validate_parameters(&self, template: &str, params: &HashMap<String, serde_json::Value>, required: &[String]) -> McpResult<()> {
        for param_name in required {
            if !params.contains_key(param_name) {
                return Err(McpError::Validation {
                    message: format!("Required parameter '{}' not provided", param_name),
                });
            }
            
            let placeholder = format!("{{{{{}}}}}", param_name);
            if !template.contains(&placeholder) {
                return Err(McpError::Validation {
                    message: format!("Parameter '{}' not found in template", param_name),
                });
            }
        }
        Ok(())
    }
}

/// Prompt registry trait for managing AI workflow templates
#[async_trait]
pub trait PromptRegistry: Send + Sync {
    /// List all available prompts
    async fn list_prompts(&self, context: &SecurityContext) -> McpResult<Vec<Prompt>>;
    
    /// Get a specific prompt by name
    async fn get_prompt(&self, name: &str, context: &SecurityContext) -> McpResult<Option<Prompt>>;
    
    /// Render a prompt with parameter substitution
    async fn get_prompt_with_args(&self, request: GetPromptRequest, context: &SecurityContext) -> McpResult<GetPromptResult>;
    
    /// List prompt categories for organization
    async fn list_categories(&self, context: &SecurityContext) -> McpResult<Vec<PromptCategory>>;
    
    /// Check if a prompt exists
    async fn prompt_exists(&self, name: &str, context: &SecurityContext) -> McpResult<bool>;
    
    /// Validate prompt parameters against schema
    async fn validate_prompt_parameters(&self, name: &str, params: &HashMap<String, serde_json::Value>, context: &SecurityContext) -> McpResult<()>;
}

/// In-memory prompt registry implementation
#[derive(Debug, Clone)]
pub struct InMemoryPromptRegistry {
    prompts: HashMap<String, Prompt>,
    categories: Vec<PromptCategory>,
    template_engine: SimpleTemplateEngine,
}

impl InMemoryPromptRegistry {
    /// Create a new in-memory prompt registry
    pub fn new() -> Self {
        Self {
            prompts: HashMap::new(),
            categories: Vec::new(),
            template_engine: SimpleTemplateEngine,
        }
    }
    
    /// Add a prompt to the registry
    pub fn add_prompt(&mut self, prompt: Prompt) {
        self.prompts.insert(prompt.name.clone(), prompt);
    }
    
    /// Add a category to the registry
    pub fn add_category(&mut self, category: PromptCategory) {
        self.categories.push(category);
    }
    
    /// Create a workflow prompt for task automation
    pub fn add_workflow_prompt(&mut self, name: &str, description: &str, system_prompt: &str, user_template: &str, parameters: Vec<PromptParameter>) {
        let prompt = Prompt {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0.0".to_string(),
            parameters,
            messages: vec![
                PromptMessage {
                    role: MessageRole::System,
                    content: PromptContent::Text {
                        text: system_prompt.to_string(),
                    },
                },
                PromptMessage {
                    role: MessageRole::User,
                    content: PromptContent::Text {
                        text: user_template.to_string(),
                    },
                },
            ],
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::Value::String("workflow".to_string()));
                meta
            },
        };
        self.add_prompt(prompt);
    }
    
    /// Create a code analysis prompt with resource embedding
    pub fn add_code_analysis_prompt(&mut self, name: &str, description: &str, resource_uri: &str) {
        let prompt = Prompt {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0.0".to_string(),
            parameters: vec![
                PromptParameter {
                    name: "analysis_type".to_string(),
                    description: "Type of analysis to perform (security, performance, style)".to_string(),
                    required: true,
                    schema: Some(serde_json::json!({
                        "type": "string",
                        "enum": ["security", "performance", "style", "all"]
                    })),
                    default: None,
                },
                PromptParameter {
                    name: "focus_areas".to_string(),
                    description: "Specific areas to focus on".to_string(),
                    required: false,
                    schema: Some(serde_json::json!({
                        "type": "array",
                        "items": {"type": "string"}
                    })),
                    default: Some(serde_json::json!([])),
                },
            ],
            messages: vec![
                PromptMessage {
                    role: MessageRole::System,
                    content: PromptContent::Text {
                        text: "You are an expert code reviewer. Analyze the provided code and give detailed feedback based on the requested analysis type.".to_string(),
                    },
                },
                PromptMessage {
                    role: MessageRole::User,
                    content: PromptContent::EmbeddedResource {
                        resource: EmbeddedResource {
                            uri: resource_uri.to_string(),
                            mime_type: Some("text/plain".to_string()),
                            annotation: Some(ResourceAnnotation {
                                description: "Source code to analyze".to_string(),
                                role: "primary_input".to_string(),
                            }),
                        },
                        text: Some("Please perform a {{analysis_type}} analysis of this code{{#if focus_areas}} focusing on: {{focus_areas}}{{/if}}. Provide specific recommendations.".to_string()),
                    },
                },
            ],
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::Value::String("code_analysis".to_string()));
                meta.insert("resource_dependent".to_string(), serde_json::Value::Bool(true));
                meta
            },
        };
        self.add_prompt(prompt);
    }
    
    /// Render a prompt message with parameter substitution
    fn render_message(&self, message: &PromptMessage, params: &HashMap<String, serde_json::Value>) -> McpResult<PromptMessage> {
        let rendered_content = match &message.content {
            PromptContent::Text { text } => {
                let rendered_text = self.template_engine.substitute(text, params)?;
                PromptContent::Text { text: rendered_text }
            },
            PromptContent::EmbeddedResource { resource, text } => {
                let rendered_text = if let Some(t) = text {
                    Some(self.template_engine.substitute(t, params)?)
                } else {
                    None
                };
                PromptContent::EmbeddedResource {
                    resource: resource.clone(),
                    text: rendered_text,
                }
            },
        };
        
        Ok(PromptMessage {
            role: message.role.clone(),
            content: rendered_content,
        })
    }
}

impl Default for InMemoryPromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PromptRegistry for InMemoryPromptRegistry {
    async fn list_prompts(&self, _context: &SecurityContext) -> McpResult<Vec<Prompt>> {
        Ok(self.prompts.values().cloned().collect())
    }
    
    async fn get_prompt(&self, name: &str, _context: &SecurityContext) -> McpResult<Option<Prompt>> {
        Ok(self.prompts.get(name).cloned())
    }
    
    async fn get_prompt_with_args(&self, request: GetPromptRequest, context: &SecurityContext) -> McpResult<GetPromptResult> {
        let prompt = self.get_prompt(&request.name, context).await?
            .ok_or_else(|| McpError::InvalidResource {
                uri: format!("prompt:{}", request.name),
                message: "Prompt not found".to_string(),
            })?;
        
        let params = request.arguments.unwrap_or_default();
        
        // Validate required parameters
        let required_params: Vec<String> = prompt.parameters
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.clone())
            .collect();
        
        for param_name in &required_params {
            if !params.contains_key(param_name) {
                return Err(McpError::Validation {
                    message: format!("Required parameter '{}' not provided", param_name),
                });
            }
        }
        
        // Render all messages with parameter substitution
        let mut rendered_messages = Vec::new();
        for message in &prompt.messages {
            let rendered_message = self.render_message(message, &params)?;
            rendered_messages.push(rendered_message);
        }
        
        // Render description
        let rendered_description = self.template_engine.substitute(&prompt.description, &params)?;
        
        Ok(GetPromptResult {
            name: request.name,
            messages: rendered_messages,
            description: rendered_description,
        })
    }
    
    async fn list_categories(&self, _context: &SecurityContext) -> McpResult<Vec<PromptCategory>> {
        Ok(self.categories.clone())
    }
    
    async fn prompt_exists(&self, name: &str, _context: &SecurityContext) -> McpResult<bool> {
        Ok(self.prompts.contains_key(name))
    }
    
    async fn validate_prompt_parameters(&self, name: &str, params: &HashMap<String, serde_json::Value>, context: &SecurityContext) -> McpResult<()> {
        let prompt = self.get_prompt(name, context).await?
            .ok_or_else(|| McpError::InvalidResource {
                uri: format!("prompt:{}", name),
                message: "Prompt not found".to_string(),
            })?;
        
        // Check required parameters
        for param in &prompt.parameters {
            if param.required && !params.contains_key(&param.name) {
                return Err(McpError::Validation {
                    message: format!("Required parameter '{}' not provided", param.name),
                });
            }
            
            // TODO: Add JSON schema validation if param.schema is provided
        }
        
        Ok(())
    }
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_template_engine() {
        let engine = SimpleTemplateEngine;
        let mut params = HashMap::new();
        params.insert("name".to_string(), serde_json::Value::String("Alice".to_string()));
        params.insert("age".to_string(), serde_json::Value::Number(serde_json::Number::from(30)));
        
        let template = "Hello {{name}}, you are {{age}} years old!";
        let result = engine.substitute(template, &params).unwrap();
        assert_eq!(result, "Hello Alice, you are 30 years old!");
    }
    
    #[test]
    fn test_template_validation() {
        let engine = SimpleTemplateEngine;
        let params = HashMap::new();
        let required = vec!["name".to_string()];
        
        let result = engine.validate_parameters("Hello {{name}}!", &params, &required);
        assert!(result.is_err());
        
        let mut params = HashMap::new();
        params.insert("name".to_string(), serde_json::Value::String("Alice".to_string()));
        let result = engine.validate_parameters("Hello {{name}}!", &params, &required);
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_prompt_registry() {
        let mut registry = InMemoryPromptRegistry::new();
        
        // Add a simple workflow prompt
        registry.add_workflow_prompt(
            "task_analyzer",
            "Analyze a task for complexity and requirements",
            "You are a task analysis expert.",
            "Analyze this task: {{task_description}}. Provide complexity rating and requirements.",
            vec![
                PromptParameter {
                    name: "task_description".to_string(),
                    description: "Description of the task to analyze".to_string(),
                    required: true,
                    schema: Some(serde_json::json!({"type": "string", "minLength": 1})),
                    default: None,
                },
            ],
        );
        
        let context = SecurityContext::system();
        
        // List prompts
        let prompts = registry.list_prompts(&context).await.unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "task_analyzer");
        
        // Get prompt with parameters
        let request = GetPromptRequest {
            name: "task_analyzer".to_string(),
            arguments: Some({
                let mut args = HashMap::new();
                args.insert("task_description".to_string(), 
                           serde_json::Value::String("Build a web scraper".to_string()));
                args
            }),
        };
        
        let result = registry.get_prompt_with_args(request, &context).await.unwrap();
        assert_eq!(result.name, "task_analyzer");
        assert_eq!(result.messages.len(), 2);
        
        // Check parameter substitution
        if let PromptContent::Text { text } = &result.messages[1].content {
            assert!(text.contains("Build a web scraper"));
        } else {
            panic!("Expected text content");
        }
    }
    
    #[tokio::test]
    async fn test_embedded_resource_prompt() {
        let mut registry = InMemoryPromptRegistry::new();
        
        registry.add_code_analysis_prompt(
            "code_reviewer",
            "Review code for {{analysis_type}} issues",
            "ratchet://tasks/code-review"
        );
        
        let context = SecurityContext::system();
        let request = GetPromptRequest {
            name: "code_reviewer".to_string(),
            arguments: Some({
                let mut args = HashMap::new();
                args.insert("analysis_type".to_string(), 
                           serde_json::Value::String("security".to_string()));
                args
            }),
        };
        
        let result = registry.get_prompt_with_args(request, &context).await.unwrap();
        assert_eq!(result.description, "Review code for security issues");
        
        // Check embedded resource
        if let PromptContent::EmbeddedResource { resource, text: _ } = &result.messages[1].content {
            assert_eq!(resource.uri, "ratchet://tasks/code-review");
            assert_eq!(resource.annotation.as_ref().unwrap().role, "primary_input");
        } else {
            panic!("Expected embedded resource content");
        }
    }
}