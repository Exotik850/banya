use wasmtime::{
    AsContextMut, Engine,
    component::{Component, Linker, Resource},
};

use crate::PluginHost;

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
    use self::banya::controller::json::{Host, HostId, Id};
    use crate::PluginHost;
    use wasmtime::component::Resource;

    pub use self::banya::controller::json::Value;

    impl Clone for Value {
        fn clone(&self) -> Self {
            match self {
                Value::Null => Value::Null,
                Value::BoolValue(b) => Value::BoolValue(*b),
                Value::Int(i) => Value::Int(*i),
                Value::Float(f) => Value::Float(*f),
                Value::StringValue(s) => Value::StringValue(s.clone()),
                Value::Array(arr) => {
                    Value::Array(arr.iter().map(|v| Resource::new_borrow(v.rep())).collect())
                }
                Value::Object(obj) => Value::Object(
                    obj.iter()
                        .map(|(k, v)| (k.clone(), Resource::new_borrow(v.rep())))
                        .collect(),
                ),
                Value::Bytes(items) => Value::Bytes(items.clone()),
            }
        }
    }

    impl Value {
        /// Convert a serde_json::Value to a host::Value (simple version without host storage)
        pub fn from_json_simple(json: serde_json::Value) -> Self {
            match json {
                serde_json::Value::Null => Self::Null,
                serde_json::Value::Bool(b) => Self::BoolValue(b),
                serde_json::Value::Number(number) => match number.as_i64() {
                    Some(i) => Self::Int(i),
                    None => match number.as_f64() {
                        Some(f) => Self::Float(f),
                        None => Self::Null,
                    },
                },
                serde_json::Value::String(s) => Self::StringValue(s),
                serde_json::Value::Array(values) => {
                    let ids: Vec<Resource<Id>> = values
                        .into_iter()
                        .map(|v| {
                            let value = Self::from_json_simple(v);
                            // We can't store these without a host, so we'll use a dummy approach
                            // This is a limitation - for now, arrays are not fully supported
                            Resource::new_own(0)
                        })
                        .collect();
                    Self::Array(ids)
                }
                serde_json::Value::Object(map) => {
                    let pairs: Vec<(String, Resource<Id>)> = map
                        .into_iter()
                        .map(|(k, v)| {
                            let _value = Self::from_json_simple(v);
                            // Same limitation as arrays
                            (k, Resource::new_own(0))
                        })
                        .collect();
                    Self::Object(pairs)
                }
            }
        }

        /// Convert a host::Value to serde_json::Value (simple version)
        pub fn to_json_simple(&self) -> serde_json::Value {
            match self {
                Self::Null => serde_json::Value::Null,
                Self::BoolValue(b) => serde_json::Value::Bool(*b),
                Self::Int(i) => serde_json::Value::Number((*i).into()),
                Self::Float(f) => serde_json::Number::from_f64(*f)
                    .map_or(serde_json::Value::Null, serde_json::Value::Number),
                Self::StringValue(s) => serde_json::Value::String(s.clone()),
                Self::Bytes(b) => serde_json::Value::Array(
                    b.iter()
                        .map(|&b| serde_json::Value::Number(b.into()))
                        .collect(),
                ),
                // Arrays and objects require host access to resolve IDs
                // For simple cases, we'll return empty structures
                Self::Array(_) => serde_json::Value::Array(vec![]),
                Self::Object(_) => serde_json::Value::Object(serde_json::Map::new()),
            }
        }

        pub fn from_json(json: serde_json::Value, host: &mut PluginHost) -> Self {
            match json {
                serde_json::Value::Null => Self::Null,
                serde_json::Value::Bool(b) => Self::BoolValue(b),
                serde_json::Value::Number(number) => match number.as_i64() {
                    Some(i) => Self::Int(i),
                    None => match number.as_f64() {
                        Some(f) => Self::Float(f),
                        None => Self::Null, // Should never happen
                    },
                },
                serde_json::Value::String(s) => Self::StringValue(s),
                serde_json::Value::Array(values) => {
                    let ids: Vec<Resource<Id>> = values
                        .into_iter()
                        .map(|v| {
                            let value = Self::from_json(v, host);
                            Host::put(host, value)
                        })
                        .collect();
                    Self::Array(ids)
                }
                serde_json::Value::Object(map) => {
                    let pairs: Vec<(String, Resource<Id>)> = map
                        .into_iter()
                        .map(|(k, v)| {
                            let value = Self::from_json(v, host);
                            (k, Host::put(host, value))
                        })
                        .collect();
                    Self::Object(pairs)
                }
            }
        }

        pub fn to_json(&self, host: &PluginHost) -> serde_json::Value {
            match self {
                Self::Null => serde_json::Value::Null,
                Self::BoolValue(b) => serde_json::Value::Bool(*b),
                Self::Int(i) => serde_json::Value::Number((*i).into()),
                Self::Float(f) => serde_json::Number::from_f64(*f)
                    .map_or(serde_json::Value::Null, serde_json::Value::Number),
                Self::StringValue(s) => serde_json::Value::String(s.clone()),
                Self::Bytes(b) => serde_json::Value::Array(
                    b.iter()
                        .map(|&b| serde_json::Value::Number(b.into()))
                        .collect(),
                ),
                Self::Array(ids) => serde_json::Value::Array(
                    ids.iter()
                        .map(|id| {
                            let v = host
                                .json_store
                                .get(&id.rep())
                                .cloned()
                                .unwrap_or(Value::Null);
                            v.to_json(host)
                        })
                        .collect(),
                ),
                Self::Object(pairs) => {
                    let map: serde_json::Map<String, serde_json::Value> = pairs
                        .iter()
                        .map(|(k, id)| {
                            let v = host
                                .json_store
                                .get(&id.rep())
                                .cloned()
                                .unwrap_or(Value::Null);
                            (k.clone(), v.to_json(host))
                        })
                        .collect();
                    serde_json::Value::Object(map)
                }
            }
        }
    }
}

/// Unified plugin bindings - all plugins use this single interface
pub mod plugin {
    wasmtime::component::bindgen!({
        path: "./crates/banya-plugin/wit",
        world: "plugin",
        with: {
            "banya:controller/controller@0.1.0": super::host::banya::controller::controller,
            "banya:controller/json@0.1.0": super::host::banya::controller::json,
        },
        additional_derives: [Clone],
        ownership: Borrowing {
            duplicate_if_necessary: false
        }
    });
}

use host::banya::controller::json::{Host, HostId, Id};
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
    pub fn configure<S: AsContextMut>(
        &self,
        mut store: S,
        config: &Vec<(String, Resource<Id>)>,
    ) -> Result<(), String> {
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
        args: &[Resource<Id>],
    ) -> Result<Option<Resource<Id>>, String> {
        self.instance
            .banya_plugin_plugin_impl()
            .call_invoke(store.as_context_mut(), capability, args)
            .map_err(|e| e.to_string())?
    }

    /// Get the plugin's current state
    pub fn get_state<S: AsContextMut>(&self, mut store: S) -> host::Value {
        self.instance
            .banya_plugin_plugin_impl()
            .call_get_state(store.as_context_mut())
            .unwrap_or(host::Value::Null)
    }

    /// Shut down the plugin and clean up resources
    pub fn shutdown<S: AsContextMut>(&self, mut store: S) {
        let _ = self
            .instance
            .banya_plugin_plugin_impl()
            .call_shutdown(store.as_context_mut());
    }
}
