//! Resource registry for MCP resources with configurable URI schemes
//!
//! This module provides a flexible resource registry system that supports
//! custom URI schemes for different projects (ratchet://, layercake://, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use url::Url;

use crate::{
    error::{McpError, McpResult},
    security::SecurityContext,
};

/// Resource content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResourceContent {
    /// Text content
    Text { text: String },
    /// Binary content (base64 encoded)
    Blob { blob: String, mime_type: String },
}

/// MCP Resource representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource URI (e.g., "ratchet://tasks/my-task")
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// MIME type of the resource
    pub mime_type: Option<String>,
    /// Resource content
    pub content: ResourceContent,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Resource template for listing available resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplate {
    /// URI template (e.g., "ratchet://tasks/{task_id}")
    pub uri_template: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// MIME type of resources created from this template
    pub mime_type: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Resource subscription for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSubscription {
    /// URI or URI pattern to subscribe to
    pub uri: String,
    /// Subscription ID for managing the subscription
    pub subscription_id: String,
}

/// Resource change notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChanged {
    /// URI of the changed resource
    pub uri: String,
    /// Type of change
    pub change_type: ResourceChangeType,
    /// Optional updated content
    pub content: Option<ResourceContent>,
}

/// Types of resource changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChangeType {
    Created,
    Updated,
    Deleted,
}

/// URI scheme configuration
#[derive(Debug, Clone)]
pub struct UriSchemeConfig {
    /// The URI scheme (e.g., "ratchet", "layercake")
    pub scheme: String,
    /// Human-readable description of the scheme
    pub description: String,
    /// Supported resource types for this scheme
    pub supported_types: Vec<String>,
}

impl UriSchemeConfig {
    /// Create a new URI scheme configuration
    pub fn new(scheme: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            description: description.into(),
            supported_types: Vec::new(),
        }
    }
    
    /// Add supported resource types
    pub fn with_types(mut self, types: Vec<String>) -> Self {
        self.supported_types = types;
        self
    }
    
    /// Check if a URI belongs to this scheme
    pub fn matches_uri(&self, uri: &str) -> bool {
        uri.starts_with(&format!("{}://", self.scheme))
    }
    
    /// Parse a URI for this scheme and extract components
    pub fn parse_uri(&self, uri: &str) -> McpResult<ParsedUri> {
        let url = Url::parse(uri).map_err(|e| McpError::InvalidResource {
            uri: uri.to_string(),
            message: format!("Invalid URI format: {}", e),
        })?;
        
        if url.scheme() != self.scheme {
            return Err(McpError::InvalidResource {
                uri: uri.to_string(),
                message: format!("Expected scheme '{}', got '{}'", self.scheme, url.scheme()),
            });
        }
        
        Ok(ParsedUri {
            scheme: url.scheme().to_string(),
            host: url.host_str().map(|s| s.to_string()),
            path: url.path().to_string(),
            query: url.query().map(|s| s.to_string()),
            fragment: url.fragment().map(|s| s.to_string()),
        })
    }
}

/// Parsed URI components
#[derive(Debug, Clone)]
pub struct ParsedUri {
    pub scheme: String,
    pub host: Option<String>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl ParsedUri {
    /// Get path segments (split by '/')
    pub fn path_segments(&self) -> Vec<&str> {
        self.path.split('/').filter(|s| !s.is_empty()).collect()
    }
    
    /// Get query parameters as key-value pairs
    pub fn query_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        if let Some(query) = &self.query {
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    params.insert(
                        urlencoding::decode(key).unwrap_or_default().to_string(),
                        urlencoding::decode(value).unwrap_or_default().to_string(),
                    );
                }
            }
        }
        params
    }
}

/// Resource registry trait for managing project-specific resources
#[async_trait]
pub trait ResourceRegistry: Send + Sync {
    /// Get the URI scheme configuration for this registry
    fn uri_scheme(&self) -> &UriSchemeConfig;
    
    /// List available resource templates
    async fn list_resource_templates(&self, context: &SecurityContext) -> McpResult<Vec<ResourceTemplate>>;
    
    /// Get a specific resource by URI
    async fn get_resource(&self, uri: &str, context: &SecurityContext) -> McpResult<Resource>;
    
    /// Check if a resource exists
    async fn resource_exists(&self, uri: &str, context: &SecurityContext) -> McpResult<bool>;
    
    /// Subscribe to resource changes
    async fn subscribe_to_resource(&self, uri: &str, context: &SecurityContext) -> McpResult<ResourceSubscription>;
    
    /// Unsubscribe from resource changes
    async fn unsubscribe_from_resource(&self, subscription_id: &str, context: &SecurityContext) -> McpResult<()>;
    
    /// Check if the registry can handle a specific URI
    fn can_handle_uri(&self, uri: &str) -> bool {
        self.uri_scheme().matches_uri(uri)
    }
}

/// Multi-scheme resource registry that delegates to scheme-specific registries
pub struct MultiSchemeResourceRegistry {
    registries: HashMap<String, Box<dyn ResourceRegistry>>,
}

impl MultiSchemeResourceRegistry {
    /// Create a new multi-scheme registry
    pub fn new() -> Self {
        Self {
            registries: HashMap::new(),
        }
    }
    
    /// Register a resource registry for a specific scheme
    pub fn register_scheme(&mut self, registry: Box<dyn ResourceRegistry>) {
        let scheme = registry.uri_scheme().scheme.clone();
        self.registries.insert(scheme, registry);
    }
    
    /// Get the registry for a specific URI scheme
    pub fn get_registry_for_uri(&self, uri: &str) -> McpResult<&dyn ResourceRegistry> {
        // Extract scheme from URI
        let scheme = if let Some(pos) = uri.find("://") {
            &uri[..pos]
        } else {
            return Err(McpError::InvalidResource {
                uri: uri.to_string(),
                message: "URI missing scheme".to_string(),
            });
        };
        
        self.registries.get(scheme)
            .map(|r| r.as_ref())
            .ok_or_else(|| McpError::InvalidResource {
                uri: uri.to_string(),
                message: format!("No registry found for scheme '{}'", scheme),
            })
    }
    
    /// List all supported schemes
    pub fn supported_schemes(&self) -> Vec<&UriSchemeConfig> {
        self.registries.values().map(|r| r.uri_scheme()).collect()
    }
}

impl Default for MultiSchemeResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ResourceRegistry for MultiSchemeResourceRegistry {
    fn uri_scheme(&self) -> &UriSchemeConfig {
        // This is a meta-registry, so we don't have a single scheme
        // In practice, callers should use get_registry_for_uri instead
        panic!("MultiSchemeResourceRegistry doesn't have a single URI scheme. Use get_registry_for_uri instead.")
    }
    
    async fn list_resource_templates(&self, context: &SecurityContext) -> McpResult<Vec<ResourceTemplate>> {
        let mut all_templates = Vec::new();
        
        for registry in self.registries.values() {
            let templates = registry.list_resource_templates(context).await?;
            all_templates.extend(templates);
        }
        
        Ok(all_templates)
    }
    
    async fn get_resource(&self, uri: &str, context: &SecurityContext) -> McpResult<Resource> {
        let registry = self.get_registry_for_uri(uri)?;
        registry.get_resource(uri, context).await
    }
    
    async fn resource_exists(&self, uri: &str, context: &SecurityContext) -> McpResult<bool> {
        let registry = self.get_registry_for_uri(uri)?;
        registry.resource_exists(uri, context).await
    }
    
    async fn subscribe_to_resource(&self, uri: &str, context: &SecurityContext) -> McpResult<ResourceSubscription> {
        let registry = self.get_registry_for_uri(uri)?;
        registry.subscribe_to_resource(uri, context).await
    }
    
    async fn unsubscribe_from_resource(&self, subscription_id: &str, context: &SecurityContext) -> McpResult<()> {
        // For unsubscription, we need to try all registries since we don't know which one owns the subscription
        for registry in self.registries.values() {
            if let Ok(()) = registry.unsubscribe_from_resource(subscription_id, context).await {
                return Ok(());
            }
        }
        
        Err(McpError::InvalidResource {
            uri: format!("subscription:{}", subscription_id),
            message: "Subscription not found in any registry".to_string(),
        })
    }
    
    fn can_handle_uri(&self, uri: &str) -> bool {
        self.get_registry_for_uri(uri).is_ok()
    }
}

/// In-memory resource registry implementation for testing
#[derive(Debug, Clone)]
pub struct InMemoryResourceRegistry {
    scheme_config: UriSchemeConfig,
    resources: HashMap<String, Resource>,
    templates: Vec<ResourceTemplate>,
    subscriptions: HashMap<String, ResourceSubscription>,
}

impl InMemoryResourceRegistry {
    /// Create a new in-memory registry with the specified scheme
    pub fn new(scheme_config: UriSchemeConfig) -> Self {
        Self {
            scheme_config,
            resources: HashMap::new(),
            templates: Vec::new(),
            subscriptions: HashMap::new(),
        }
    }
    
    /// Add a resource to the registry
    pub fn add_resource(&mut self, resource: Resource) {
        self.resources.insert(resource.uri.clone(), resource);
    }
    
    /// Add a resource template
    pub fn add_template(&mut self, template: ResourceTemplate) {
        self.templates.push(template);
    }
}

#[async_trait]
impl ResourceRegistry for InMemoryResourceRegistry {
    fn uri_scheme(&self) -> &UriSchemeConfig {
        &self.scheme_config
    }
    
    async fn list_resource_templates(&self, _context: &SecurityContext) -> McpResult<Vec<ResourceTemplate>> {
        Ok(self.templates.clone())
    }
    
    async fn get_resource(&self, uri: &str, _context: &SecurityContext) -> McpResult<Resource> {
        self.resources.get(uri)
            .cloned()
            .ok_or_else(|| McpError::ResourceNotFound {
                uri: uri.to_string(),
            })
    }
    
    async fn resource_exists(&self, uri: &str, _context: &SecurityContext) -> McpResult<bool> {
        Ok(self.resources.contains_key(uri))
    }
    
    async fn subscribe_to_resource(&self, uri: &str, _context: &SecurityContext) -> McpResult<ResourceSubscription> {
        let subscription = ResourceSubscription {
            uri: uri.to_string(),
            subscription_id: uuid::Uuid::new_v4().to_string(),
        };
        Ok(subscription)
    }
    
    async fn unsubscribe_from_resource(&self, _subscription_id: &str, _context: &SecurityContext) -> McpResult<()> {
        // In-memory implementation just accepts all unsubscriptions
        Ok(())
    }
}

impl fmt::Display for ResourceChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceChangeType::Created => write!(f, "created"),
            ResourceChangeType::Updated => write!(f, "updated"),
            ResourceChangeType::Deleted => write!(f, "deleted"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_scheme_config() {
        let scheme = UriSchemeConfig::new("ratchet", "Ratchet task management")
            .with_types(vec!["task".to_string(), "execution".to_string()]);
        
        assert_eq!(scheme.scheme, "ratchet");
        assert!(scheme.matches_uri("ratchet://tasks/my-task"));
        assert!(!scheme.matches_uri("layercake://models/my-model"));
    }
    
    #[test]
    fn test_uri_parsing() {
        let scheme = UriSchemeConfig::new("ratchet", "Ratchet");
        let parsed = scheme.parse_uri("ratchet://host/path/to/resource?param=value#fragment").unwrap();
        
        assert_eq!(parsed.scheme, "ratchet");
        assert_eq!(parsed.host, Some("host".to_string()));
        assert_eq!(parsed.path, "/path/to/resource");
        assert_eq!(parsed.query, Some("param=value".to_string()));
        assert_eq!(parsed.fragment, Some("fragment".to_string()));
        
        let segments = parsed.path_segments();
        assert_eq!(segments, vec!["path", "to", "resource"]);
        
        let params = parsed.query_params();
        assert_eq!(params.get("param"), Some(&"value".to_string()));
    }
    
    #[tokio::test]
    async fn test_in_memory_registry() {
        let scheme = UriSchemeConfig::new("test", "Test scheme");
        let mut registry = InMemoryResourceRegistry::new(scheme);
        
        let resource = Resource {
            uri: "test://example/resource".to_string(),
            name: "Test Resource".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            content: ResourceContent::Text { text: "Hello, world!".to_string() },
            metadata: HashMap::new(),
        };
        
        registry.add_resource(resource.clone());
        
        let context = SecurityContext::system();
        let retrieved = registry.get_resource("test://example/resource", &context).await.unwrap();
        assert_eq!(retrieved.uri, resource.uri);
        assert_eq!(retrieved.name, resource.name);
        
        let exists = registry.resource_exists("test://example/resource", &context).await.unwrap();
        assert!(exists);
        
        let not_exists = registry.resource_exists("test://example/nonexistent", &context).await.unwrap();
        assert!(!not_exists);
    }
    
    #[tokio::test]
    async fn test_multi_scheme_registry() {
        let mut multi_registry = MultiSchemeResourceRegistry::new();
        
        // Register ratchet scheme
        let ratchet_scheme = UriSchemeConfig::new("ratchet", "Ratchet tasks");
        let ratchet_registry = InMemoryResourceRegistry::new(ratchet_scheme);
        multi_registry.register_scheme(Box::new(ratchet_registry));
        
        // Register layercake scheme
        let layercake_scheme = UriSchemeConfig::new("layercake", "Layercake models");
        let layercake_registry = InMemoryResourceRegistry::new(layercake_scheme);
        multi_registry.register_scheme(Box::new(layercake_registry));
        
        // Test scheme resolution
        assert!(multi_registry.can_handle_uri("ratchet://tasks/task1"));
        assert!(multi_registry.can_handle_uri("layercake://models/model1"));
        assert!(!multi_registry.can_handle_uri("unknown://something"));
        
        let schemes = multi_registry.supported_schemes();
        assert_eq!(schemes.len(), 2);
        let scheme_names: Vec<&str> = schemes.iter().map(|s| s.scheme.as_str()).collect();
        assert!(scheme_names.contains(&"ratchet"));
        assert!(scheme_names.contains(&"layercake"));
    }
}