pub mod logical;
pub mod registry;

// Re-export common types for convenience
pub use registry::{
    ClosureNativeFunction, NativeCapabilityBuilder, NativeCapabilityHandler,
    NativeCapabilityRoute, NativeCapabilitySchema, NativeFunction, NativeFunctionInfo,
    NativeFunctionInfoBuilder, NativeFunctionRegistry, NativeInvokeResult, RoutedNativeFunction,
    dispatch_native_capability, native_capability, native_capability_route,
    native_function_info, native_function_info_from_routes,
};
