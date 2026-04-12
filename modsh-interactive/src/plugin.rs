//! Plugin system — WASM-based plugin sandbox

use thiserror::Error;

/// Plugin errors
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("invalid plugin format: {0}")]
    InvalidFormat(String),
    #[error("plugin execution failed: {0}")]
    ExecutionFailed(String),
}

/// Plugin manifest
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Author
    pub author: String,
    /// Entry point
    pub entry: String,
    /// Hooks provided
    pub hooks: Vec<String>,
}

/// A loaded plugin
#[derive(Debug)]
pub struct Plugin {
    /// Manifest
    pub manifest: PluginManifest,
    /// Plugin path
    pub path: std::path::PathBuf,
}

/// Plugin manager
pub struct PluginManager {
    plugins: Vec<Plugin>,
    plugin_dir: std::path::PathBuf,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(plugin_dir: std::path::PathBuf) -> Self {
        Self {
            plugins: Vec::new(),
            plugin_dir,
        }
    }

    /// Load all plugins from the plugin directory
    pub fn load_all(&mut self) -> Result<(), PluginError> {
        if !self.plugin_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.plugin_dir).map_err(|e| {
            PluginError::InvalidFormat(e.to_string())
        })? {
            let entry = entry.map_err(|e| PluginError::InvalidFormat(e.to_string()))?;
            let path = entry.path();

            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                // TODO: Load WASM plugin
            } else if path.is_dir() {
                // Look for manifest
                let manifest_path = path.join("modsh-plugin.toml");
                if manifest_path.exists() {
                    // TODO: Parse manifest and load plugin
                }
            }
        }

        Ok(())
    }

    /// Install a plugin
    pub fn install(&mut self, _source: &str) -> Result<(), PluginError> {
        // TODO: Download and install plugin from:
        // - Local path
        // - Git repository
        // - Plugin registry
        todo!("Plugin installation not yet implemented")
    }

    /// Remove a plugin
    pub fn remove(&mut self, name: &str) -> Result<(), PluginError> {
        let pos = self.plugins
            .iter()
            .position(|p| p.manifest.name == name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        let plugin = self.plugins.remove(pos);
        
        // Remove plugin files
        if let Err(e) = std::fs::remove_dir_all(&plugin.path) {
            return Err(PluginError::ExecutionFailed(e.to_string()));
        }

        Ok(())
    }

    /// List installed plugins
    pub fn list(&self) -> &[Plugin] {
        &self.plugins
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.manifest.name == name)
    }

    /// Execute a plugin hook
    pub fn execute_hook(&self, hook: &str, context: &str) -> Result<String, PluginError> {
        // TODO: Execute hook on all plugins that provide it
        for plugin in &self.plugins {
            if plugin.manifest.hooks.contains(&hook.to_string()) {
                // TODO: Call WASM function
            }
        }
        Ok(context.to_string())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        let plugin_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("modsh/plugins");
        Self::new(plugin_dir)
    }
}

/// Plugin directory helper
mod dirs {
    pub fn data_dir() -> Option<std::path::PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| std::path::PathBuf::from(h).join(".local/share"))
            })
    }
}
