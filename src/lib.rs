use std::{collections::HashMap, path::Path, sync::Arc};

use wasmtime::{
    AsContextMut, Engine,
    component::{Access, HasData, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use crate::bindings::Plugin;

pub mod bindings;
pub mod instruction;

/// The host state that manages all loaded plugins and provides WASI context
pub struct PluginHost {
    table: ResourceTable,
    ctx: WasiCtx,
    pub plugins: HashMap<String, Arc<Plugin>>,
    data_store: HashMap<String, Vec<u8>>,
}

impl PluginHost {
    /// Create a new `PluginHost` with the given WASI context
    #[must_use]
    pub fn new(ctx: WasiCtx) -> Self {
        Self {
            table: ResourceTable::new(),
            ctx,
            plugins: HashMap::new(),
            data_store: HashMap::new(),
        }
    }

    /// Add a plugin to the host's registry
    pub fn push(&mut self, plugin: Plugin) {
        self.plugins
            .insert(plugin.name().to_string(), Arc::new(plugin));
    }

    /// Get a plugin by name
    #[must_use] 
    pub fn get(&self, name: &str) -> Option<&Arc<Plugin>> {
        self.plugins.get(name)
    }

    /// Get all loaded plugins
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<Plugin>)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get the number of loaded plugins
    #[must_use] 
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if any plugins are loaded
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Load a plugin from a file path
    pub fn load_file<S: AsContextMut>(
        &mut self,
        path: impl AsRef<Path>,
        engine: &Engine,
        store: S,
        linker: &Linker<S::Data>,
    ) -> Result<(), String> {
        let path = path.as_ref();
        let plugin = Plugin::from_file(path, engine, store, linker)
            .map_err(|e| format!("Failed to load plugin '{}': {e}", path.display()))?;
        let name = plugin.name().to_string();
        self.plugins.insert(name.clone(), Arc::new(plugin));
        Ok(())
    }

    /// Find plugins that have a specific capability
    #[must_use] 
    pub fn find_by_capability(&self, capability: &str) -> Vec<&Arc<Plugin>> {
        self.plugins
            .values()
            .filter(|p| p.has_capability(capability))
            .collect()
    }

    /// Find a plugin by name that has a specific capability
    #[must_use] 
    pub fn find_plugin_with_capability(
        &self,
        name: &str,
        capability: &str,
    ) -> Option<&Arc<Plugin>> {
        self.plugins
            .get(name)
            .filter(|p| p.has_capability(capability))
    }

    /// Shut down all plugins gracefully
    pub fn shutdown_all<S: AsContextMut>(&mut self, mut store: S) {
        for plugin in self.plugins.values() {
            plugin.shutdown(store.as_context_mut());
        }
        self.plugins.clear();
    }
}

impl WasiView for PluginHost {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

/// Implement the controller host trait - allows plugins to call back into the host
impl bindings::host::banya::controller::controller::Host for PluginHost {
    fn get(&mut self, name: String) -> Option<Vec<u8>> {
        self.data_store.get(&name).cloned()
    }

    fn put(&mut self, name: String, data: Vec<u8>) -> Option<Vec<u8>> {
        self.data_store.insert(name, data)
    }

    fn remove(&mut self, name: String) -> Option<Vec<u8>> {
        self.data_store.remove(&name)
    }
}

impl HasData for PluginHost {
    type Data<'a> = &'a mut PluginHost;
}

/// Allow plugins to execute other plugins via the controller interface
impl bindings::host::banya::controller::controller::HostWithStore for PluginHost {
    fn execute<T>(
        mut host: Access<T, Self>,
        name: String,
        capability: String,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        let plugin = {
            let host_data = host.get();
            match host_data.plugins.get(&name) {
                Some(plugin) => Arc::clone(plugin),
                None => return Err(format!("No plugin found for action: {name}")),
            }
        };

        plugin.invoke(host.as_context_mut(), &capability, &data)
    }
}
