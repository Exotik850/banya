pub mod bindings {
    wit_bindgen::generate!({
        path: "./wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "banya_plugin::bindings",
        generate_all,
        ownership: Borrowing {
          duplicate_if_necessary: true
        }
    });
}

pub use bindings::exports::banya::plugin::plugin_impl::{CapabilitySchema, Guest, PluginInfo};
pub use serde_json::{Value as JsonValue, json};

pub type PluginResult = Result<Option<JsonValue>, String>;

/// High-level plugin authoring API.
///
/// Implement this trait to avoid manual JSON byte handling and to opt into
/// default `configure`, `state`, and `shutdown` behavior.
pub trait JsonPlugin {
  /// Return plugin metadata and capability schema.
  fn info() -> PluginInfo;

  /// Handle capability invocation with JSON arguments.
  fn invoke(capability: &str, args: JsonValue) -> PluginResult;

  /// Optionally handle configuration payload.
  fn configure(_config: JsonValue) -> Result<(), String> {
    Ok(())
  }

  /// Optionally expose runtime state.
  fn state() -> JsonValue {
    JsonValue::Null
  }

  /// Optionally clean up resources on plugin unload.
  fn shutdown() {}
}

pub type PluginCapabilityHandler = fn(JsonValue) -> PluginResult;

#[derive(Clone)]
pub struct CapabilityRoute {
  capability: CapabilitySchema,
  handler: PluginCapabilityHandler,
}

#[must_use]
pub fn capability_route<C>(capability: C, handler: PluginCapabilityHandler) -> CapabilityRoute
where
  C: Into<CapabilitySchema>,
{
  CapabilityRoute {
    capability: capability.into(),
    handler,
  }
}

impl CapabilityRoute {
  #[must_use]
  pub fn capability(&self) -> &CapabilitySchema {
    &self.capability
  }
}

#[must_use]
pub fn plugin_info_from_routes(builder: PluginInfoBuilder, routes: &[CapabilityRoute]) -> PluginInfo {
  builder
    .capabilities(routes.iter().map(|route| route.capability.clone()))
    .build()
}

pub fn dispatch_plugin_capability(
  capability: &str,
  args: JsonValue,
  routes: &[CapabilityRoute],
) -> PluginResult {
  routes
    .iter()
    .find(|route| route.capability.name == capability)
    .map_or_else(
      || Err(format!("Unknown capability: {capability}")),
      |route| (route.handler)(args),
    )
}

/// A declarative plugin API where capabilities are registered as routes.
///
/// Implement this trait to define capability schemas and handlers in one place,
/// while still getting the `JsonPlugin` interface exported over WIT.
pub trait RoutedJsonPlugin {
  /// Return plugin metadata without capabilities.
  fn info_builder() -> PluginInfoBuilder;

  /// Return capability schemas and handlers.
  fn capability_routes() -> Vec<CapabilityRoute>;

  /// Optionally handle configuration payload.
  fn configure(_config: JsonValue) -> Result<(), String> {
    Ok(())
  }

  /// Optionally expose runtime state.
  fn state() -> JsonValue {
    JsonValue::Null
  }

  /// Optionally clean up resources on plugin unload.
  fn shutdown() {}
}

impl<T: RoutedJsonPlugin> JsonPlugin for T {
  fn info() -> PluginInfo {
    plugin_info_from_routes(T::info_builder(), &T::capability_routes())
  }

  fn invoke(capability: &str, args: JsonValue) -> PluginResult {
    dispatch_plugin_capability(capability, args, &T::capability_routes())
  }

  fn configure(config: JsonValue) -> Result<(), String> {
    T::configure(config)
  }

  fn state() -> JsonValue {
    T::state()
  }

  fn shutdown() {
    T::shutdown();
  }
}

impl<T: JsonPlugin> Guest for T {
  fn init() -> PluginInfo {
    T::info()
  }

  fn configure(config: Vec<u8>) -> Result<(), String> {
    T::configure(json_from_bytes(&config)?)
  }

  fn invoke(capability: String, args: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
    let args = json_from_bytes(&args)?;
    let value = T::invoke(&capability, args)?;
    value.map(|v| json_to_bytes(&v)).transpose()
  }

  fn get_state() -> Vec<u8> {
    json_to_bytes(&T::state()).unwrap_or_else(|_| json_null_bytes())
  }

  fn shutdown() {
    T::shutdown();
  }
}

/// Parse JSON bytes used by the WIT boundary.
pub fn json_from_bytes(bytes: &[u8]) -> Result<JsonValue, String> {
  if bytes.is_empty() {
    return Ok(JsonValue::Null);
  }
  serde_json::from_slice(bytes).map_err(|e| format!("Failed to parse JSON bytes: {e}"))
}

/// Serialize a JSON value for the WIT boundary.
pub fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
  serde_json::to_vec(value).map_err(|e| format!("Failed to serialize JSON: {e}"))
}

/// A static `null` payload for callers that need a byte vector.
#[must_use]
pub fn json_null_bytes() -> Vec<u8> {
  b"null".to_vec()
}

/// Helpers for extracting common argument types from JSON objects.
pub trait JsonArgsExt {
  fn require_object(&self) -> Result<&serde_json::Map<String, JsonValue>, String>;
  fn require_str(&self, key: &str) -> Result<&str, String>;
  fn optional_str(&self, key: &str) -> Option<&str>;
  fn require_array(&self, key: &str) -> Result<&Vec<JsonValue>, String>;
}

impl JsonArgsExt for JsonValue {
  fn require_object(&self) -> Result<&serde_json::Map<String, JsonValue>, String> {
    self.as_object()
      .ok_or_else(|| format!("Expected JSON object args, got {self:?}"))
  }

  fn require_str(&self, key: &str) -> Result<&str, String> {
    let map = self.require_object()?;
    let value = map
      .get(key)
      .ok_or_else(|| format!("Missing required argument '{key}'"))?;
    value
      .as_str()
      .ok_or_else(|| format!("Argument '{key}' expected string, got {value:?}"))
  }

  fn optional_str(&self, key: &str) -> Option<&str> {
    self.as_object()?.get(key)?.as_str()
  }

  fn require_array(&self, key: &str) -> Result<&Vec<JsonValue>, String> {
    let map = self.require_object()?;
    let value = map
      .get(key)
      .ok_or_else(|| format!("Missing required argument '{key}'"))?;
    value
      .as_array()
      .ok_or_else(|| format!("Argument '{key}' expected array, got {value:?}"))
  }
}

#[derive(Debug, Clone)]
pub struct CapabilityBuilder {
  name: String,
  description: Option<String>,
  inputs: Vec<(String, String)>,
  output: Option<String>,
}

#[must_use]
pub fn capability(name: impl Into<String>) -> CapabilityBuilder {
  CapabilityBuilder {
    name: name.into(),
    description: None,
    inputs: Vec::new(),
    output: None,
  }
}

impl CapabilityBuilder {
  #[must_use]
  pub fn description(mut self, description: impl Into<String>) -> Self {
    self.description = Some(description.into());
    self
  }

  #[must_use]
  pub fn input(mut self, name: impl Into<String>, ty: impl Into<String>) -> Self {
    self.inputs.push((name.into(), ty.into()));
    self
  }

  #[must_use]
  pub fn inputs<I, N, T>(mut self, inputs: I) -> Self
  where
    I: IntoIterator<Item = (N, T)>,
    N: Into<String>,
    T: Into<String>,
  {
    self.inputs
      .extend(inputs.into_iter().map(|(n, t)| (n.into(), t.into())));
    self
  }

  #[must_use]
  pub fn output(mut self, output: impl Into<String>) -> Self {
    self.output = Some(output.into());
    self
  }

  #[must_use]
  pub fn build(self) -> CapabilitySchema {
    CapabilitySchema {
      name: self.name,
      description: self.description,
      inputs: self.inputs,
      output: self.output,
    }
  }
}

impl From<CapabilityBuilder> for CapabilitySchema {
  fn from(value: CapabilityBuilder) -> Self {
    value.build()
  }
}

#[derive(Debug, Clone)]
pub struct PluginInfoBuilder {
  name: String,
  version: String,
  description: Option<String>,
  author: Option<String>,
  capabilities: Vec<CapabilitySchema>,
}

#[must_use]
pub fn plugin_info(name: impl Into<String>, version: impl Into<String>) -> PluginInfoBuilder {
  PluginInfoBuilder {
    name: name.into(),
    version: version.into(),
    description: None,
    author: None,
    capabilities: Vec::new(),
  }
}

impl PluginInfoBuilder {
  #[must_use]
  pub fn description(mut self, description: impl Into<String>) -> Self {
    self.description = Some(description.into());
    self
  }

  #[must_use]
  pub fn author(mut self, author: impl Into<String>) -> Self {
    self.author = Some(author.into());
    self
  }

  #[must_use]
  pub fn capability<C>(mut self, capability: C) -> Self
  where
    C: Into<CapabilitySchema>,
  {
    self.capabilities.push(capability.into());
    self
  }

  #[must_use]
  pub fn capabilities<I, C>(mut self, capabilities: I) -> Self
  where
    I: IntoIterator<Item = C>,
    C: Into<CapabilitySchema>,
  {
    self.capabilities
      .extend(capabilities.into_iter().map(Into::into));
    self
  }

  #[must_use]
  pub fn build(self) -> PluginInfo {
    PluginInfo {
      name: self.name,
      version: self.version,
      description: self.description,
      author: self.author,
      capabilities: self.capabilities,
    }
  }
}

/// Re-export common plugin authoring symbols.
pub mod prelude {
  pub use crate::bindings;
  pub use crate::capability;
  pub use crate::capability_route;
  pub use crate::dispatch_plugin_capability;
  pub use crate::export_plugin;
  pub use crate::json;
  pub use crate::json_from_bytes;
  pub use crate::json_null_bytes;
  pub use crate::json_to_bytes;
  pub use crate::plugin_info_from_routes;
  pub use crate::plugin_info;
  pub use crate::CapabilityBuilder;
  pub use crate::CapabilityRoute;
  pub use crate::CapabilitySchema;
  pub use crate::JsonArgsExt;
  pub use crate::JsonPlugin;
  pub use crate::JsonValue;
  pub use crate::PluginCapabilityHandler;
  pub use crate::PluginInfo;
  pub use crate::PluginInfoBuilder;
  pub use crate::PluginResult;
  pub use crate::RoutedJsonPlugin;
}

#[macro_export]
macro_rules! export_plugin {
  ($plugin:ident) => {
    $crate::bindings::export!($plugin);
  };
}
