use banya_plugin::bindings::exports::banya::plugin::plugin_impl::{
    CapabilitySchema, Guest, PluginInfo,
};
use serde_json::Value as JsonValue;

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
    fn configure(config: Vec<u8>) -> Result<(), String> {
        let config_value = json_from_bytes(&config)?;
        let config_object = match config_value {
            JsonValue::Null => return Ok(()),
            JsonValue::Object(map) => map,
            other => return Err(format!("Config must be an object, got {other:?}")),
        };

        for (key, value) in config_object {
            println!("  [PrintTime] Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(capability: String, args: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        match capability.as_str() {
            "action" => {
                let json_obj = json_from_bytes(&args)?;

                let time = extract_string_from_object(&json_obj, "time")?;
                let message = extract_string_from_object(&json_obj, "message")
                    .unwrap_or_else(|_| "Current time".to_string());

                println!("  [PrintTime] {}: {}", message, time);

                let result = format!("Printed: {} - {}", message, time);
                let result = JsonValue::String(result);
                Ok(Some(json_to_bytes(&result)?))
            }

            _ => Err(format!("Unknown capability: {}", capability)),
        }
    }

    /// Return the plugin's current state
    fn get_state() -> Vec<u8> {
        let state = serde_json::json!({
            "name": "print-time",
            "version": "0.1.0",
            "status": "running"
        });
        json_to_bytes(&state).unwrap_or_else(|_| json_null_bytes())
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [PrintTime Plugin] Shutting down gracefully");
    }
}

/// Helper to extract a string value from a JSON object
fn extract_string_from_object(value: &JsonValue, key: &str) -> Result<String, String> {
    match value {
        JsonValue::Object(map) => match map.get(key) {
            Some(JsonValue::String(s)) => Ok(s.clone()),
            Some(other) => Err(format!("Key '{key}' must be a string, got {other:?}")),
            None => Err(format!("Key '{key}' not found in object")),
        },
        other => Err(format!("Expected object, got {other:?}")),
    }
}

fn json_from_bytes(bytes: &[u8]) -> Result<JsonValue, String> {
    if bytes.is_empty() {
        return Ok(JsonValue::Null);
    }
    serde_json::from_slice(bytes).map_err(|e| format!("Failed to parse JSON bytes: {e}"))
}

fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| format!("Failed to serialize JSON: {e}"))
}

fn json_null_bytes() -> Vec<u8> {
    b"null".to_vec()
}
