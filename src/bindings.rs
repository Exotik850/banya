use wasmtime::{
    AsContextMut, Engine,
    component::{Component, Linker},
};

pub mod host {
    wasmtime::component::bindgen!({
      path: "./wit",
      imports: {
        "banya:controller/controller.execute": store
      }
    });
}
pub mod sensor {
    wasmtime::component::bindgen!({
      path: "./crates/banya-plugin/wit",
      world: "sensor-plugin",
      with: {
        "banya:controller/controller@0.1.0": super::host::banya::controller::controller,
      }
    });
}
pub mod action {
    wasmtime::component::bindgen!({
      path: "./crates/banya-plugin/wit",
      world: "action-plugin",
      with: {
        "banya:controller/controller@0.1.0": super::host::banya::controller::controller,
      }
    });
}

pub struct Plugin {
    name: String,
    inner: PluginKind,
}

pub enum PluginKind {
    Sensor(sensor::SensorPlugin),
    Action(action::ActionPlugin),
}

impl Plugin {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instantiate<S: AsContextMut>(
        name: String,
        mut store: S,
        component: &Component,
        sensor_linker: &Linker<S::Data>,
        action_linker: &Linker<S::Data>,
    ) -> Result<Self, wasmtime::Error> {
        let inner = sensor::SensorPlugin::instantiate(&mut store, component, sensor_linker)
            .map(PluginKind::Sensor)
            .or_else(|e| {
                println!("Failed to instantiate sensor plugin: {e}");
                action::ActionPlugin::instantiate(store, component, action_linker)
                    .map(PluginKind::Action)
            })?;
        Ok(Self { name, inner })
    }

    pub fn execute(&self, store: impl AsContextMut, data: String) -> Result<String, String> {
        match &self.inner {
            PluginKind::Sensor(sensor) => sensor
                .banya_plugin_sensor()
                .call_matches(store, &data)
                .map(|res| res.to_string())
                .map_err(|e| e.to_string()),
            PluginKind::Action(action_plugin) => {
                let res = action_plugin
                    .banya_plugin_action()
                    .call_execute(store, &data)
                    .map_err(|e| e.to_string())?;
                res.map_err(|e| e.clone())
            }
        }
    }

    pub fn from_file<S: AsContextMut>(
        path: impl AsRef<std::path::Path>,
        engine: &Engine,
        store: S,
        sensor_linker: &Linker<S::Data>,
        action_linker: &Linker<S::Data>,
    ) -> Result<Self, wasmtime::Error> {
        let name = path
            .as_ref()
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| wasmtime::Error::msg("File name unable to be used as name!"))?
            .to_string();
        let component = Component::from_file(engine, path)?;
        Self::instantiate(name, store, &component, sensor_linker, action_linker)
    }
}
