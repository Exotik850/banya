use std::{collections::HashMap, path::Path, sync::Arc};

use wasmtime::{
    AsContextMut, Engine,
    component::{Access, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use crate::bindings::Plugin;

pub mod bindings;
pub mod instruction;

pub struct PluginHost {
    table: ResourceTable,
    ctx: WasiCtx,
    plugins: HashMap<String, Arc<Plugin>>,
}

impl PluginHost {
    #[must_use]
    pub fn new(ctx: WasiCtx) -> Self {
        Self {
            table: ResourceTable::new(),
            ctx,
            plugins: HashMap::new(),
        }
    }

    pub fn push(&mut self, plugin: Plugin) {
        self.plugins
            .insert(plugin.name().to_string(), Arc::new(plugin));
    }

    pub fn load_file<S: AsContextMut>(
        &mut self,
        path: impl AsRef<Path>,
        engine: &Engine,
        store: S,
        sensor_linker: &Linker<S::Data>,
        action_linker: &Linker<S::Data>,
    ) -> Result<(), String> {
        let path = path.as_ref();
        let filename = path.file_name().and_then(|f| f.to_str()).ok_or_else(|| {
            format!(
                "Invalid plugin path: {} (must have a valid filename)",
                path.display()
            )
        })?;
        let plugin = Plugin::from_file(path, engine, store, sensor_linker, action_linker)
            .map_err(|e| format!("Failed to load plugin: {e}"))?;
        self.plugins.insert(filename.to_string(), Arc::new(plugin));
        Ok(())
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

impl bindings::host::banya::controller::controller::Host for PluginHost {}
impl wasmtime::component::HasData for PluginHost {
    type Data<'a> = &'a mut PluginHost;
}

impl bindings::host::banya::controller::controller::HostWithStore for PluginHost {
    fn execute<T>(mut host: Access<T, Self>, name: String, data: String) -> Result<String, String> {
        let plugin = {
            let host_data = host.get();
            match host_data.plugins.get(&name) {
                Some(plugin) => Arc::clone(plugin),
                None => return Err(format!("No plugin found for action: {name}")),
            }
        };
        plugin.execute(host.as_context_mut(), data)
    }
}
