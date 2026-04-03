use banya_plugin::bindings::{
    banya::controller::json::{Id, Value as JsonValue},
    exports::{self, banya::plugin::plugin_impl::{CapabilitySchema, Guest, PluginInfo, Value}},
};

/// PrintTime plugin - prints a time value to the console
pub struct PrintTimePlugin;
banya_plugin::bindings::export!(PrintTimePlugin);

impl Guest for PrintTimePlugin {
    /// Initialize and return plugin metadata
    fn init() -> PluginInfo {
        PluginInfo {
            name: "print-time".into(),
            version: "0.1.0".into(),
            description: Some("A plugin that prints a time value to the console".into()),
            author: Some("Banya Team".into()),
            capabilities: vec![CapabilitySchema {
                name: "action".into(),
                description: Some("Prints a time value with an optional prefix message".into()),
                inputs: vec![
                    ("time".into(), "string".into()),
                    ("message".into(), "string".into()),
                ],
                output: Some("string".into()),
            }],
        }
    }

    /// Configure the plugin (no configuration needed)
    fn configure(config: Vec<(String, Id)>) -> Result<(), String> {
        for (key, id) in &config {
            let value = id.get();
            println!("  [PrintTime] Config: {} = {:?}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(capability: String, args: Vec<Id>) -> Result<Option<Id>, String> {
        match capability.as_str() {
            "action" => {
                // Get the JSON object from the first (and only) argument
                let json_obj = args.first().ok_or("Missing arguments object")?.get();

                let time = extract_string_from_object(&json_obj, "time")?;
                let message = extract_string_from_object(&json_obj, "message")
                    .unwrap_or_else(|_| "Current time".to_string());

                println!("  [PrintTime] {}: {}", message, time);

                let result = format!("Printed: {} - {}", message, time);
                let id = JsonValue::StringValue(result).into();
                Ok(Some(id))
            }

            _ => Err(format!("Unknown capability: {}", capability)),
        }
    }

    /// Return the plugin's current state
    fn get_state() -> Value {
        JsonValue::Object(vec![
            (
                "name".into(),
                JsonValue::StringValue("print-time".into()).into(),
            ),
            (
                "version".into(),
                JsonValue::StringValue("0.1.0".into()).into(),
            ),
            (
                "status".into(),
                JsonValue::StringValue("running".into()).into(),
            ),
        ])
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [PrintTime Plugin] Shutting down gracefully");
    }
}

/// Helper to extract a string value from a JSON object
fn extract_string_from_object(value: &JsonValue, key: &str) -> Result<String, String> {
    match value {
        JsonValue::Object(pairs) => {
            for (k, v) in pairs {
                if k == key {
                    return match v.into() {
                        JsonValue::StringValue(s) => Ok(s.clone()),
                        other => Err(format!("Key '{key}' must be a string, got {:?}", other)),
                    };
                }
            }
            Err(format!("Key '{key}' not found in object"))
        }
        other => Err(format!("Expected object, got {:?}", other)),
    }
}
