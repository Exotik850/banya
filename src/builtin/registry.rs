use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::bindings::plugin::exports::banya::plugin::plugin_impl::{
    CapabilitySchema as NativeCapabilitySchema, PluginInfo as NativeFunctionInfo,
};

#[derive(Debug, Clone)]
pub struct NativeCapabilityBuilder {
    name: String,
    description: Option<String>,
    inputs: Vec<(String, String)>,
    output: Option<String>,
}

#[must_use]
pub fn native_capability(name: impl Into<String>) -> NativeCapabilityBuilder {
    NativeCapabilityBuilder {
        name: name.into(),
        description: None,
        inputs: Vec::new(),
        output: None,
    }
}

impl NativeCapabilityBuilder {
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
    pub fn build(self) -> NativeCapabilitySchema {
        NativeCapabilitySchema {
            name: self.name,
            description: self.description,
            inputs: self.inputs,
            output: self.output,
        }
    }
}

impl From<NativeCapabilityBuilder> for NativeCapabilitySchema {
    fn from(value: NativeCapabilityBuilder) -> Self {
        value.build()
    }
}

#[derive(Debug, Clone)]
pub struct NativeFunctionInfoBuilder {
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
    capabilities: Vec<NativeCapabilitySchema>,
}

#[must_use]
pub fn native_function_info(
    name: impl Into<String>,
    version: impl Into<String>,
) -> NativeFunctionInfoBuilder {
    NativeFunctionInfoBuilder {
        name: name.into(),
        version: version.into(),
        description: None,
        author: None,
        capabilities: Vec::new(),
    }
}

impl NativeFunctionInfoBuilder {
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
        C: Into<NativeCapabilitySchema>,
    {
        self.capabilities.push(capability.into());
        self
    }

    #[must_use]
    pub fn capabilities<I, C>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<NativeCapabilitySchema>,
    {
        self.capabilities
            .extend(capabilities.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn build(self) -> NativeFunctionInfo {
        NativeFunctionInfo {
            name: self.name,
            version: self.version,
            description: self.description,
            author: self.author,
            capabilities: self.capabilities,
        }
    }
}

pub type NativeInvokeResult = Result<JsonValue, String>;
pub type NativeCapabilityHandler<T> = fn(&T, &JsonValue) -> NativeInvokeResult;

#[derive(Clone)]
pub struct NativeCapabilityRoute<T> {
    capability: NativeCapabilitySchema,
    handler: NativeCapabilityHandler<T>,
}

#[must_use]
pub fn native_capability_route<T, C>(
    capability: C,
    handler: NativeCapabilityHandler<T>,
) -> NativeCapabilityRoute<T>
where
    C: Into<NativeCapabilitySchema>,
{
    NativeCapabilityRoute {
        capability: capability.into(),
        handler,
    }
}

impl<T> NativeCapabilityRoute<T> {
    #[must_use]
    pub fn capability(&self) -> &NativeCapabilitySchema {
        &self.capability
    }
}

#[must_use]
pub fn native_function_info_from_routes<T>(
    builder: NativeFunctionInfoBuilder,
    routes: &[NativeCapabilityRoute<T>],
) -> NativeFunctionInfo {
    builder
        .capabilities(routes.iter().map(|route| route.capability.clone()))
        .build()
}

pub fn dispatch_native_capability<T>(
    function: &T,
    capability: &str,
    args: &JsonValue,
    routes: &[NativeCapabilityRoute<T>],
) -> NativeInvokeResult {
    routes
        .iter()
        .find(|route| route.capability.name == capability)
        .map_or_else(
            || Err(format!("Unknown capability: {capability}")),
            |route| (route.handler)(function, args),
        )
}

type NativeInvokeFn = dyn Fn(&str, &JsonValue) -> NativeInvokeResult + Send + Sync;

/// A native function backed by a closure for quick host-side extensions.
pub struct ClosureNativeFunction {
    info: NativeFunctionInfo,
    handler: Box<NativeInvokeFn>,
}

impl ClosureNativeFunction {
    #[must_use]
    pub fn new<F>(info: NativeFunctionInfo, handler: F) -> Self
    where
        F: Fn(&str, &JsonValue) -> NativeInvokeResult + Send + Sync + 'static,
    {
        Self {
            info,
            handler: Box::new(handler),
        }
    }
}

impl NativeFunction for ClosureNativeFunction {
    fn info(&self) -> NativeFunctionInfo {
        self.info.clone()
    }

    fn invoke(&self, capability: &str, args: &JsonValue) -> NativeInvokeResult {
        (self.handler)(capability, args)
    }
}

/// Trait for compile-time native functions that can be invoked like WASM plugins.
///
/// Native functions execute directly in the host process, giving them access to
/// system resources, GPU handles, window management, game state, and other
/// capabilities that WASM plugins cannot access. They use the same JSON-based
/// interface as plugins, making them transparent to end users.
///
/// # Example
/// ```rust
/// pub struct WindowManager {
///     // handles to window system
/// }
///
/// impl NativeFunction for WindowManager {
///     fn info(&self) -> NativeFunctionInfo {
///         NativeFunctionInfo {
///             name: "window-manager".to_string(),
///             version: "1.0.0".to_string(),
///             description: Some("Manage application windows".to_string()),
///             author: Some("developer".to_string()),
///             capabilities: vec![NativeCapabilitySchema {
///                 name: "action".to_string(),
///                 description: Some("Perform window operations".to_string()),
///                 inputs: vec![("operation".to_string(), "string".to_string())],
///                 output: Some("object".to_string()),
///             }],
///         }
///     }
///
///     fn invoke(&self, capability: &str, args: &JsonValue) -> Result<JsonValue, String> {
///         match capability {
///             "action" => {
///                 let op = args.get("operation")
///                     .and_then(|v| v.as_str())
///                     .ok_or("Missing 'operation' argument")?;
///                 match op {
///                     "minimize_all" => Ok(json!({"status": "minimized"})),
///                     _ => Err(format!("Unknown operation: {op}")),
///                 }
///             }
///             _ => Err(format!("Unknown capability: {capability}")),
///         }
///     }
/// }
/// ```
pub trait NativeFunction: Send + Sync {
    /// Return metadata about this native function
    fn info(&self) -> NativeFunctionInfo;

    /// Invoke a capability with JSON arguments.
    ///
    /// This is the main entry point for executing native function logic.
    /// Unlike WASM plugins, this method has direct access to any state
    /// the function holds, enabling complex operations like GPU compute,
    /// window management, or game state manipulation.
    ///
    /// # Arguments
    /// * `capability` - The capability to invoke (e.g., "action", "sensor")
    /// * `args` - JSON arguments for the capability
    ///
    /// # Returns
    /// * `Ok(JsonValue)` - The result of the capability invocation
    /// * `Err(String)` - Error message if the capability failed
    fn invoke(&self, capability: &str, args: &JsonValue) -> NativeInvokeResult;

    /// Get the function's unique name
    fn name(&self) -> String {
        self.info().name
    }

    /// Check if this function has a specific capability
    fn has_capability(&self, capability: &str) -> bool {
        self.info()
            .capabilities
            .iter()
            .any(|c| c.name == capability)
    }
}

/// Ergonomic native function API that routes capabilities through a declarative table.
pub trait RoutedNativeFunction: Send + Sync + Sized {
    /// Return function metadata without capabilities.
    fn info_builder(&self) -> NativeFunctionInfoBuilder;

    /// Return capability schemas and handlers.
    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>>;
}

impl<T> NativeFunction for T
where
    T: RoutedNativeFunction + Send + Sync,
{
    fn info(&self) -> NativeFunctionInfo {
        native_function_info_from_routes(self.info_builder(), &self.capability_routes())
    }

    fn invoke(&self, capability: &str, args: &JsonValue) -> NativeInvokeResult {
        dispatch_native_capability(self, capability, args, &self.capability_routes())
    }
}

/// Registry of compile-time native functions
#[derive(Default)]
pub struct NativeFunctionRegistry {
    functions: HashMap<String, Arc<dyn NativeFunction>>,
}

impl NativeFunctionRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a native function
    pub fn register<F: NativeFunction + 'static>(&mut self, function: F) {
        let name = function.name();
        self.functions.insert(name, Arc::new(function));
    }

    /// Register a boxed native function.
    pub fn register_boxed(&mut self, function: Box<dyn NativeFunction>) {
        let name = function.name();
        self.functions.insert(name, Arc::from(function));
    }

    /// Register a native function backed by a closure.
    pub fn register_closure<F>(&mut self, info: NativeFunctionInfo, handler: F)
    where
        F: Fn(&str, &JsonValue) -> NativeInvokeResult + Send + Sync + 'static,
    {
        self.register(ClosureNativeFunction::new(info, handler));
    }

    /// Get a native function by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn NativeFunction>> {
        self.functions.get(name).cloned()
    }

    /// Check if a native function exists
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Get all registered function names
    #[must_use]
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(String::as_str)
    }

    /// Get the number of registered functions
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Find functions that have a specific capability
    #[must_use]
    pub fn find_by_capability(&self, capability: &str) -> Vec<Arc<dyn NativeFunction>> {
        self.functions
            .values()
            .filter(|f| f.has_capability(capability))
            .cloned()
            .collect()
    }
}

#[macro_export]
macro_rules! register_native_functions {
    ($host:expr, $($function:expr),+ $(,)?) => {{
        $(
            $host.register_native($function);
        )+
    }};
}
