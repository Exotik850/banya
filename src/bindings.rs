use wasmtime::{
    AsContextMut, Engine,
    component::{Component, Linker},
};

/// Host-side bindings for the controller interface that plugins can import
pub mod host {
    wasmtime::component::bindgen!({
        path: "./wit",
        imports: {
            "banya:controller/controller.execute": store
        },
        ownership: Borrowing {
            duplicate_if_necessary: false
        }
    });
}

/// Unified plugin bindings - all plugins use this single interface
pub mod plugin {
    wasmtime::component::bindgen!({
        path: "./crates/banya-plugin/wit",
        world: "plugin",
        with: {
            "banya:controller/controller@0.1.0": super::host::banya::controller::controller,
        },
        additional_derives: [Clone],
        ownership: Borrowing {
            duplicate_if_necessary: false
        }
    });
}

use plugin::exports::banya::plugin::plugin_impl::CapabilitySchema;

/// A unified plugin that can be any type (sensor, action, transform, etc.)
/// Plugins declare their capabilities via metadata, and the host invokes them dynamically
pub struct Plugin {
    name: String,
    version: String,
    capabilities: Vec<CapabilitySchema>,
    instance: plugin::Plugin,
}

impl Plugin {
    /// Get the plugin's unique name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the plugin's version string
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get the list of capabilities this plugin provides
    #[must_use]
    pub fn capabilities(&self) -> &[CapabilitySchema] {
        &self.capabilities
    }

    /// Check if this plugin has a specific capability
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == capability)
    }

    /// Instantiate a plugin from a pre-loaded component
    pub fn instantiate<S: AsContextMut>(
        mut store: S,
        component: &Component,
        linker: &Linker<S::Data>,
    ) -> Result<Self, wasmtime::Error> {
        let instance = plugin::Plugin::instantiate(store.as_context_mut(), component, linker)?;

        // Call init to get plugin metadata
        let info = instance
            .banya_plugin_plugin_impl()
            .call_init(store.as_context_mut())
            .map_err(|e| wasmtime::Error::msg(format!("Plugin init failed: {e}")))?;

        let name = info.name.clone();
        let version = info.version.clone();
        let capabilities = info.capabilities.clone();

        Ok(Self {
            name,
            version,
            capabilities,
            instance,
        })
    }

    /// Load and instantiate a plugin from a .wasm file
    pub fn from_file<S: AsContextMut>(
        path: impl AsRef<std::path::Path>,
        engine: &Engine,
        store: S,
        linker: &Linker<S::Data>,
    ) -> Result<Self, wasmtime::Error> {
        let component = Component::from_file(engine, path.as_ref())?;
        Self::instantiate(store, &component, linker)
    }

    /// Configure the plugin with key-value settings
    pub fn configure<S: AsContextMut>(&self, mut store: S, config: &Vec<u8>) -> Result<(), String> {
        self.instance
            .banya_plugin_plugin_impl()
            .call_configure(store.as_context_mut(), config)
            .map_err(|e| e.to_string())?
    }

    /// Invoke a plugin capability with arguments
    pub fn invoke<S: AsContextMut>(
        &self,
        mut store: S,
        capability: &str,
        args: &Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        self.instance
            .banya_plugin_plugin_impl()
            .call_invoke(store.as_context_mut(), capability, args)
            .map_err(|e| e.to_string())?
    }

    /// Get the plugin's current state
    pub fn get_state<S: AsContextMut>(&self, mut store: S) -> Vec<u8> {
        self.instance
            .banya_plugin_plugin_impl()
            .call_get_state(store.as_context_mut())
            .unwrap_or_else(|_| b"null".to_vec())
    }

    /// Shut down the plugin and clean up resources
    pub fn shutdown<S: AsContextMut>(&self, mut store: S) {
        let _ = self
            .instance
            .banya_plugin_plugin_impl()
            .call_shutdown(store.as_context_mut());
    }
}
