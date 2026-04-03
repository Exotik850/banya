use banya_plugin::prelude::*;

/// Echo plugin - demonstrates the unified plugin interface
/// This plugin provides both "sensor" and "action" capabilities
pub struct EchoPlugin;
export_plugin!(EchoPlugin);

impl EchoPlugin {
    fn sensor(args: JsonValue) -> PluginResult {
        let data = args.require_str("data")?;
        println!("  [Echo Sensor] Received data: {}", data);
        let matches = data == "value";
        let result = JsonValue::Bool(matches);
        Ok(Some(result))
    }

    fn action(args: JsonValue) -> PluginResult {
        let data = args.require_str("data")?;
        println!("  [Echo Action] Executing with data: {}", data);
        let result = format!("Echo: {data}");
        let result = JsonValue::String(result);
        Ok(Some(result))
    }
}

impl RoutedJsonPlugin for EchoPlugin {
    /// Initialize and return plugin metadata without capability schemas
    fn info_builder() -> PluginInfoBuilder {
        plugin_info("echo", "0.1.0")
            .description("A simple echo plugin that demonstrates the unified plugin API")
            .author("Banya Team")
    }

    fn capability_routes() -> Vec<CapabilityRoute> {
        vec![
            capability_route(
                capability("sensor")
                    .description("Checks if input data matches a specific value")
                    .input("data", "string")
                    .output("bool"),
                Self::sensor,
            ),
            capability_route(
                capability("action")
                    .description("Echoes back the input data with a prefix")
                    .input("data", "string")
                    .output("string"),
                Self::action,
            ),
        ]
    }

    /// Configure the plugin (echo plugin doesn't need configuration)
    fn configure(config: JsonValue) -> Result<(), String> {
        let config_object = match config {
            JsonValue::Null => return Ok(()),
            JsonValue::Object(map) => map,
            other => return Err(format!("Config must be an object, got {other:?}")),
        };

        for (key, value) in config_object {
            println!("  Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Return the plugin's current state (echo plugin has no state)
    fn state() -> JsonValue {
        json!({
            "name": "echo",
            "version": "0.1.0",
            "status": "running"
        })
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [Echo Plugin] Shutting down gracefully");
    }
}
