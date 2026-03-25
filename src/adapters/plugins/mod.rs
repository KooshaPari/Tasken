//! Plugin system for extensibility.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Plugin trait for extending taskkit.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name.
    fn name(&self) -> &str;

    /// Plugin version.
    fn version(&self) -> &str;

    /// Initialize the plugin.
    async fn init(&self) -> Result<(), String> {
        Ok(())
    }

    /// Shutdown the plugin.
    async fn shutdown(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub config: serde_json::Value,
}

impl PluginConfig {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            enabled: true,
            config: serde_json::Value::Null,
        }
    }

    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Plugin registry for managing plugins.
pub struct PluginRegistry {
    plugins: std::collections::HashMap<String, Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: std::collections::HashMap::new(),
        }
    }

    /// Register a plugin.
    pub fn register(&mut self, plugin: impl Plugin + 'static) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, Box::new(plugin));
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// List all plugin names.
    pub fn names(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin;

    #[async_trait]
    impl Plugin for TestPlugin {
        fn name(&self) -> &str {
            "test-plugin"
        }

        fn version(&self) -> &str {
            "0.1.0"
        }
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();
        registry.register(TestPlugin);

        assert!(registry.get("test-plugin").is_some());
        assert_eq!(registry.names(), vec!["test-plugin"]);
    }
}
