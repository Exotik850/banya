use banya_plugin::prelude::*;

/// PrintTime plugin - prints a time value to the console
pub struct PrintTimePlugin;
export_plugin!(PrintTimePlugin);

impl PrintTimePlugin {
    fn action(args: JsonValue) -> PluginResult {
        let time = args.require_str("time")?;
        let message = args.optional_str("message").unwrap_or("Current time");

        println!("  [PrintTime] {}: {}", message, time);

        let result = format!("Printed: {} - {}", message, time);
        let result = JsonValue::String(result);
        Ok(Some(result))
    }
}

impl RoutedJsonPlugin for PrintTimePlugin {
    /// Initialize and return plugin metadata without capability schemas
    fn info_builder() -> PluginInfoBuilder {
        plugin_info("print-time", "0.1.0")
            .description("A plugin that prints a time value to the console")
            .author("Banya Team")
    }

    fn capability_routes() -> Vec<CapabilityRoute> {
        vec![capability_route(
                capability("action")
                    .description("Prints a time value with an optional prefix message")
                    .inputs([("time", "string"), ("message", "string")])
                    .output("string"),
                Self::action,
            )]
    }

    /// Configure the plugin (no configuration needed)
    fn configure(config: JsonValue) -> Result<(), String> {
        let config_object = match config {
            JsonValue::Null => return Ok(()),
            JsonValue::Object(map) => map,
            other => return Err(format!("Config must be an object, got {other:?}")),
        };

        for (key, value) in config_object {
            println!("  [PrintTime] Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Return the plugin's current state
    fn state() -> JsonValue {
        json!({
            "name": "print-time",
            "version": "0.1.0",
            "status": "running"
        })
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [PrintTime Plugin] Shutting down gracefully");
    }
}
