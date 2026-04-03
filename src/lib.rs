use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, atomic::AtomicU32},
};

use wasmtime::{
    AsContextMut, Engine,
    component::{Access, HasData, Linker, Resource},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use crate::bindings::{Plugin, host::Value, host::banya::controller::json::Id};

pub mod bindings;
pub mod instruction;

/// The host state that manages all loaded plugins and provides WASI context
pub struct PluginHost {
    table: ResourceTable,
    ctx: WasiCtx,
    pub plugins: HashMap<String, Arc<Plugin>>,
    current_id: AtomicU32,
    pub json_store: HashMap<u32, Value>,
    json_map: HashMap<String, u32>,
}

impl PluginHost {
    /// Create a new PluginHost with the given WASI context
    #[must_use]
    pub fn new(ctx: WasiCtx) -> Self {
        Self {
            table: ResourceTable::new(),
            ctx,
            plugins: HashMap::new(),
            json_store: HashMap::new(),
            json_map: HashMap::new(),
            current_id: AtomicU32::new(0),
        }
    }

    /// Add a plugin to the host's registry
    pub fn push(&mut self, plugin: Plugin) {
        self.plugins
            .insert(plugin.name().to_string(), Arc::new(plugin));
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&Arc<Plugin>> {
        self.plugins.get(name)
    }

    /// Get all loaded plugins
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<Plugin>)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get the number of loaded plugins
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if any plugins are loaded
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
    pub fn find_by_capability(&self, capability: &str) -> Vec<&Arc<Plugin>> {
        self.plugins
            .values()
            .filter(|p| p.has_capability(capability))
            .collect()
    }

    /// Find a plugin by name that has a specific capability
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

    fn next_id(&self) -> u32 {
        self.current_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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
impl bindings::host::banya::controller::controller::Host for PluginHost {}

impl HasData for PluginHost {
    type Data<'a> = &'a mut PluginHost;
}

/// Allow plugins to execute other plugins via the controller interface
impl bindings::host::banya::controller::controller::HostWithStore for PluginHost {
    fn execute<T>(
        mut host: Access<T, Self>,
        name: String,
        capability: String,
        data: Resource<Id>,
    ) -> Result<Option<Resource<Id>>, String> {
        let plugin = {
            let host_data = host.get();
            match host_data.plugins.get(&name) {
                Some(plugin) => Arc::clone(plugin),
                None => return Err(format!("No plugin found for action: {name}")),
            }
        };

        plugin.invoke(host.as_context_mut(), &capability, &[data])
    }
}

impl bindings::host::banya::controller::json::HostId for PluginHost {
    fn get(&mut self, self_: Resource<Id>) -> Value {
        let id = self_.rep();
        self.json_store.get(&id).cloned().unwrap_or(Value::Null)
    }

    fn drop(&mut self, rep: Resource<Id>) -> wasmtime::Result<()> {
        self.json_store.remove(&rep.rep());
        Ok(())
    }
}
impl bindings::host::banya::controller::json::Host for PluginHost {
    fn put(&mut self, val: Value) -> Resource<Id> {
        let id = self.next_id();
        self.json_store.insert(id, val);
        Resource::new_borrow(id)
    }

    fn put_named(&mut self, name: String, val: Value) -> (Resource<Id>, Option<Value>) {
        let id = self.next_id();
        self.json_store.insert(id, val);
        let old_val = self
            .json_map
            .insert(name.to_string(), id)
            .and_then(|old_id| self.json_store.remove(&old_id));
        (Resource::new_borrow(id), old_val)
    }

    fn get(&mut self, name: String) -> Option<Resource<Id>> {
        self.json_map.get(&name).map(|id| Resource::new_borrow(*id))
    }

    fn remove(&mut self, name: String) -> Option<Value> {
        self.json_map
            .remove(&name)
            .and_then(|id| self.json_store.remove(&id))
    }
}
