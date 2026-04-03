use banya_plugin::bindings::exports::banya::plugin::plugin_impl::{
    CapabilitySchema, Guest, PluginInfo,
};
use serde_json::Value as JsonValue;

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
            description: Some(
                "A simple echo plugin that demonstrates the unified plugin API".into(),
            ),
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
    fn configure(config: Vec<u8>) -> Result<(), String> {
        let config_value = json_from_bytes(&config)?;
        let config_object = match config_value {
            JsonValue::Null => return Ok(()),
            JsonValue::Object(map) => map,
            other => return Err(format!("Config must be an object, got {other:?}")),
        };

        for (key, value) in config_object {
            println!("  Config: {} = {}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(capability: String, args: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        let args_value = json_from_bytes(&args)?;
        match capability.as_str() {
            // Sensor capability: check if data matches "value"
            "sensor" => {
                let data = extract_string_arg(&args_value, "data")?;
                println!("  [Echo Sensor] Received data: {}", data);
                let matches = data == "value";
                let result = JsonValue::Bool(matches);
                Ok(Some(json_to_bytes(&result)?))
            }

            // Action capability: echo back the input
            "action" => {
                let data = extract_string_arg(&args_value, "data")?;
                println!("  [Echo Action] Executing with data: {}", data);
                let result = format!("Echo: {data}");
                let result = JsonValue::String(result);
                Ok(Some(json_to_bytes(&result)?))
            }

            // Unknown capability
            _ => Err(format!("Unknown capability: {}", capability)),
        }
    }

    /// Return the plugin's current state (echo plugin has no state)
    fn get_state() -> Vec<u8> {
        let state = serde_json::json!({
            "name": "echo",
            "version": "0.1.0",
            "status": "running"
        });
        json_to_bytes(&state).unwrap_or_else(|_| json_null_bytes())
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [Echo Plugin] Shutting down gracefully");
    }
}

fn extract_string_arg(value: &JsonValue, name: &str) -> Result<String, String> {
    match value {
        JsonValue::Object(map) => match map.get(name) {
            Some(JsonValue::String(s)) => Ok(s.clone()),
            Some(other) => Err(format!("Argument '{name}' expected string, got {other:?}")),
            None => Err(format!("Missing required argument '{name}'")),
        },
        JsonValue::Array(values) => match values.first() {
            Some(JsonValue::String(s)) => Ok(s.clone()),
            Some(other) => Err(format!("Argument '{name}' expected string, got {other:?}")),
            None => Err(format!("Missing required argument '{name}'")),
        },
        JsonValue::String(s) => Ok(s.clone()),
        other => Err(format!("Argument '{name}' expected string, got {other:?}")),
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
