use banya_plugin::bindings::{
    exports::banya::plugin::plugin_impl::{
        CapabilitySchema, Guest, PluginInfo,
    },
    banya::controller::json::{Id, Value as JsonValue},
};

/// Echo plugin - demonstrates the unified plugin interface
/// This plugin provides both "sensor" and "action" capabilities
pub struct EchoPlugin;
banya_plugin::bindings::export!(EchoPlugin);

impl Guest for EchoPlugin {
    /// Initialize and return plugin metadata
    fn init() -> PluginInfo {
        PluginInfo {
            name: "echo".into(),
            version: "0.1.0".into(),
            description: Some("A simple echo plugin that demonstrates the unified plugin API".into()),
            author: Some("Banya Team".into()),
            capabilities: vec![
                CapabilitySchema {
                    name: "sensor".into(),
                    description: Some("Checks if input data matches a specific value".into()),
                    inputs: vec![("data".into(), "string".into())],
                    output: Some("bool".into()),
                },
                CapabilitySchema {
                    name: "action".into(),
                    description: Some("Echoes back the input data with a prefix".into()),
                    inputs: vec![("data".into(), "string".into())],
                    output: Some("string".into()),
                },
            ],
        }
    }

    /// Configure the plugin (echo plugin doesn't need configuration)
    fn configure(config: Vec<(String, Id)>) -> Result<(), String> {
        // Log configuration entries (in a real plugin, you'd store these)
        for (key, id) in &config {
            let value = id.get();
            println!("  Config: {} = {:?}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(
        capability: String,
        args: Vec<Id>,
    ) -> Result<Option<Id>, String> {
        match capability.as_str() {
            // Sensor capability: check if data matches "value"
            "sensor" => {
                let data = extract_string_arg(&args, 0, "data")?;
                println!("  [Echo Sensor] Received data: {}", data);
                let matches = data == "value";
                let id = JsonValue::BoolValue(matches).into();
                Ok(Some(id))
            }

            // Action capability: echo back the input
            "action" => {
                let data = extract_string_arg(&args, 0, "data")?;
                println!("  [Echo Action] Executing with data: {}", data);
                let result = format!("Echo: {data}");
                let id = JsonValue::StringValue(result).into();
                Ok(Some(id))
            }

            // Unknown capability
            _ => Err(format!(
                "Unknown capability: {}",
                capability
            )),
        }
    }

    /// Return the plugin's current state (echo plugin has no state)
    fn get_state() -> JsonValue {
        JsonValue::Object(vec![
            ("name".into(), JsonValue::StringValue("echo".into()).into()),
            ("version".into(), JsonValue::StringValue("0.1.0".into()).into()),
            ("status".into(), JsonValue::StringValue("running".into()).into()),
        ])
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [Echo Plugin] Shutting down gracefully");
    }
}

/// Helper to extract a string argument from the args list
fn extract_string_arg(args: &[Id], index: usize, name: &str) -> Result<String, String> {
    args.get(index)
        .ok_or_else(|| format!("Missing required argument '{name}' at index {index}"))
        .and_then(|id| {
            let value = id.get();
            match value {
                JsonValue::StringValue(s) => Ok(s),
                other => Err(format!(
                    "Argument '{name}' expected string, got {:?}",
                    other
                )),
            }
        })
}
