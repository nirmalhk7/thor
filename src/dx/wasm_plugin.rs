use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// WASM Plugin Metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<String>,
}

/// Simulated lightweight WASM Plugin Instance sandbox
#[derive(Debug, Clone)]
pub struct WasmPluginInstance {
    pub manifest: WasmPluginManifest,
    pub is_enabled: bool,
}

/// Dynamic WASM Microkernel Plugin Manager for Thor
#[derive(Debug, Clone, Default)]
pub struct WasmPluginMicrokernel {
    plugins: Arc<RwLock<HashMap<String, WasmPluginInstance>>>,
}

impl WasmPluginMicrokernel {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register and load a WASM plugin module
    pub fn register_plugin(&self, manifest: WasmPluginManifest) -> Result<(), String> {
        let id = manifest.id.clone();
        let instance = WasmPluginInstance {
            manifest,
            is_enabled: true,
        };

        if let Ok(mut lock) = self.plugins.write() {
            lock.insert(id, instance);
            Ok(())
        } else {
            Err("Failed to acquire write lock for WASM plugin registry".to_string())
        }
    }

    /// Toggle plugin state
    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> bool {
        if let Ok(mut lock) = self.plugins.write() {
            if let Some(plugin) = lock.get_mut(plugin_id) {
                plugin.is_enabled = enabled;
                return true;
            }
        }
        false
    }

    /// Get list of active plugins
    pub fn get_active_plugins(&self) -> Vec<WasmPluginManifest> {
        if let Ok(lock) = self.plugins.read() {
            lock.values()
                .filter(|p| p.is_enabled)
                .map(|p| p.manifest.clone())
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_plugin_registration_and_toggle() {
        let kernel = WasmPluginMicrokernel::new();

        let manifest = WasmPluginManifest {
            id: "jwt_cracker_wasm".to_string(),
            name: "WASM JWT Secret Cracker".to_string(),
            version: "1.0.0".to_string(),
            description: "Cracks weak HMAC JWT signatures inside WASM sandbox".to_string(),
            author: "Thor Security".to_string(),
            capabilities: vec!["crypto".to_string(), "decoder".to_string()],
        };

        assert!(kernel.register_plugin(manifest).is_ok());
        assert_eq!(kernel.get_active_plugins().len(), 1);

        assert!(kernel.set_plugin_enabled("jwt_cracker_wasm", false));
        assert_eq!(kernel.get_active_plugins().len(), 0);
    }
}
