use std::{collections::HashMap, path::Path, sync::Arc};

use serde_json::Value as JsonValue;
use wasmtime::{
    AsContextMut, Engine,
    component::{Access, HasData, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use crate::bindings::Plugin;
use crate::builtin::registry::NativeFunctionRegistry;

pub mod bindings;
pub mod builtin;
pub mod instruction;

/// The host state that manages all loaded plugins and provides WASI context
pub struct PluginHost {
    table: ResourceTable,
    ctx: WasiCtx,
    pub(crate) plugins: HashMap<String, Arc<Plugin>>,
    pub(crate) native_functions: NativeFunctionRegistry,
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
            native_functions: NativeFunctionRegistry::new(),
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

    /// Get the number of registered native functions
    #[must_use]
    pub fn len_native(&self) -> usize {
        self.native_functions.len()
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

    /// Register a compile-time native function
    pub fn register_native<F: crate::builtin::registry::NativeFunction + 'static>(
        &mut self,
        function: F,
    ) {
        self.native_functions.register(function);
    }

    /// Register a boxed native function.
    pub fn register_native_boxed(
        &mut self,
        function: Box<dyn crate::builtin::registry::NativeFunction>,
    ) {
        self.native_functions.register_boxed(function);
    }

    /// Register a native function backed by a closure.
    pub fn register_native_closure<F>(
        &mut self,
        info: crate::builtin::registry::NativeFunctionInfo,
        handler: F,
    ) where
        F: Fn(&str, &JsonValue) -> Result<JsonValue, String> + Send + Sync + 'static,
    {
        self.native_functions.register_closure(info, handler);
    }

    /// Get a native function by name
    #[must_use]
    pub fn get_native(
        &self,
        name: &str,
    ) -> Option<Arc<dyn crate::builtin::registry::NativeFunction>> {
        self.native_functions.get(name)
    }

    /// Check if a name refers to a WASM plugin or a native function
    #[must_use]
    pub fn resolve_callable(&self, name: &str) -> Option<CallableTarget> {
        if let Some(plugin) = self.plugins.get(name) {
            Some(CallableTarget::Plugin(Arc::clone(plugin)))
        } else if let Some(func) = self.native_functions.get(name) {
            Some(CallableTarget::Native(func))
        } else {
            None
        }
    }

    pub fn plugin_names(&self) -> impl Iterator<Item = &str> {
        self.plugins.keys().map(|k| k.as_str())
    }

    pub fn native_function_names(&self) -> impl Iterator<Item = &str> {
        self.native_functions.names()
    }

    pub fn all_names(&self) -> impl Iterator<Item = &str> {
        self.plugin_names().chain(self.native_function_names())
    }
}

/// Represents what a callable name resolves to
#[derive(Clone)]
pub enum CallableTarget {
    Plugin(Arc<Plugin>),
    Native(Arc<dyn crate::builtin::registry::NativeFunction>),
}

impl CallableTarget {
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        match self {
            Self::Plugin(plugin) => plugin.has_capability(capability),
            Self::Native(function) => function.has_capability(capability),
        }
    }

    #[must_use]
    pub fn capability_names(&self) -> Vec<String> {
        match self {
            Self::Plugin(plugin) => plugin
                .capabilities()
                .iter()
                .map(|c| c.name.clone())
                .collect(),
            Self::Native(function) => function
                .info()
                .capabilities
                .into_iter()
                .map(|c| c.name)
                .collect(),
        }
    }

    pub fn invoke_json<S: AsContextMut>(
        &self,
        mut store: S,
        capability: &str,
        args: &JsonValue,
    ) -> Result<JsonValue, String> {
        match self {
            Self::Plugin(plugin) => {
                let args_bytes = serde_json::to_vec(args)
                    .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
                let result = plugin.invoke(&mut store, capability, &args_bytes)?;

                match result {
                    Some(bytes) if bytes.is_empty() => Ok(JsonValue::Null),
                    Some(bytes) => serde_json::from_slice(&bytes)
                        .map_err(|e| format!("Failed to parse JSON bytes: {e}")),
                    None => Ok(JsonValue::Null),
                }
            }
            Self::Native(function) => function.invoke(capability, args),
        }
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
        let callable = {
            let host_data = host.get();
            host_data.resolve_callable(&name)
        };

        let args = if data.is_empty() {
            JsonValue::Null
        } else {
            serde_json::from_slice::<JsonValue>(&data)
                .map_err(|e| format!("Failed to parse controller.execute JSON payload: {e}"))?
        };

        let result = callable
            .ok_or_else(|| format!("No plugin or native function found for action: {name}"))?
            .invoke_json(host.as_context_mut(), &capability, &args)?;

        if result.is_null() {
            Ok(None)
        } else {
            serde_json::to_vec(&result)
                .map(Some)
                .map_err(|e| format!("Failed to serialize native function result: {e}"))
        }
    }
}
