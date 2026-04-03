use banya_plugin::prelude::*;
use chrono::Local;

/// GetTime plugin - returns the current time in various formats
pub struct GetTimePlugin;
export_plugin!(GetTimePlugin);

impl GetTimePlugin {
    fn sensor(args: JsonValue) -> PluginResult {
        // Extract format from the JSON object (default to ISO 8601)
        let format = args.optional_str("format").unwrap_or("%Y-%m-%d %H:%M:%S");

        let now = Local::now();
        let time_str = now.format(&format).to_string();

        println!("  [GetTime] Current time: {}", time_str);

        let result = JsonValue::String(time_str);
        Ok(Some(result))
    }
}

impl RoutedJsonPlugin for GetTimePlugin {
    /// Initialize and return plugin metadata without capability schemas
    fn info_builder() -> PluginInfoBuilder {
        plugin_info("get-time", "0.1.0")
            .description("A plugin that retrieves the current system time")
            .author("Banya Team")
    }

    fn capability_routes() -> Vec<CapabilityRoute> {
        vec![capability_route(
                capability("sensor")
                    .description("Returns the current time in the specified format")
                    .input("format", "string")
                    .output("string"),
                Self::sensor,
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
            println!("  [GetTime] Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Return the plugin's current state
    fn state() -> JsonValue {
        let now = Local::now();
        json!({
            "name": "get-time",
            "version": "0.1.0",
            "current_time": now.to_rfc3339(),
            "status": "running"
        })
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [GetTime Plugin] Shutting down gracefully");
    }
}
