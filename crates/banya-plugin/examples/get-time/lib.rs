use banya_plugin::bindings::exports::banya::plugin::plugin_impl::{
    CapabilitySchema, Guest, PluginInfo,
};
use chrono::Local;
use serde_json::Value as JsonValue;

/// GetTime plugin - returns the current time in various formats
pub struct GetTimePlugin;
banya_plugin::bindings::export!(GetTimePlugin);

impl Guest for GetTimePlugin {
    /// Initialize and return plugin metadata
    fn init() -> PluginInfo {
        PluginInfo {
            name: "get-time".into(),
            version: "0.1.0".into(),
            description: Some("A plugin that retrieves the current system time".into()),
            author: Some("Banya Team".into()),
            capabilities: vec![CapabilitySchema {
                name: "sensor".into(),
                description: Some("Returns the current time in the specified format".into()),
                inputs: vec![("format".into(), "string".into())],
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
            println!("  [GetTime] Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(capability: String, args: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        match capability.as_str() {
            "sensor" => {
                let json_obj = json_from_bytes(&args)?;

                // Extract format from the JSON object (default to ISO 8601)
                let format = extract_string_from_object(&json_obj, "format")
                    .unwrap_or_else(|_| "%Y-%m-%d %H:%M:%S".to_string());

                let now = Local::now();
                let time_str = now.format(&format).to_string();

                println!("  [GetTime] Current time: {}", time_str);

                let result = JsonValue::String(time_str);
                Ok(Some(json_to_bytes(&result)?))
            }

            _ => Err(format!("Unknown capability: {}", capability)),
        }
    }

    /// Return the plugin's current state
    fn get_state() -> Vec<u8> {
        let now = Local::now();
        let state = serde_json::json!({
            "name": "get-time",
            "version": "0.1.0",
            "current_time": now.to_rfc3339(),
            "status": "running"
        });
        json_to_bytes(&state).unwrap_or_else(|_| json_null_bytes())
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [GetTime Plugin] Shutting down gracefully");
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
