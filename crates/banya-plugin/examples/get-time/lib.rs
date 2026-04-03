use banya_plugin::bindings::{
    banya::controller::json::{Id, Value as JsonValue},
    exports::banya::plugin::plugin_impl::{CapabilitySchema, Guest, PluginInfo, Value},
};
use chrono::Local;

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
    fn configure(config: Vec<(String, Id)>) -> Result<(), String> {
        for (key, id) in &config {
            let value = id.get();
            println!("  [GetTime] Config: {} = {:?}", key, value);
        }
        Ok(())
    }

    /// Invoke a capability dynamically
    fn invoke(capability: String, args: Vec<Id>) -> Result<Option<Id>, String> {
        match capability.as_str() {
            "sensor" => {
                // Get the JSON object from the first (and only) argument
                let json_obj = args.first().ok_or("Missing arguments object")?.get();

                // Extract format from the JSON object (default to ISO 8601)
                let format = extract_string_from_object(&json_obj, "format")
                    .unwrap_or_else(|_| "%Y-%m-%d %H:%M:%S".to_string());

                let now = Local::now();
                let time_str = now.format(&format).to_string();

                println!("  [GetTime] Current time: {}", time_str);

                let id = JsonValue::StringValue(time_str).into();
                Ok(Some(id))
            }

            _ => Err(format!("Unknown capability: {}", capability)),
        }
    }

    /// Return the plugin's current state
    fn get_state() -> Value {
        let now = Local::now();
        JsonValue::Object(vec![
            (
                "name".into(),
                JsonValue::StringValue("get-time".into()).into(),
            ),
            (
                "version".into(),
                JsonValue::StringValue("0.1.0".into()).into(),
            ),
            (
                "current_time".into(),
                JsonValue::StringValue(now.to_rfc3339()).into(),
            ),
            (
                "status".into(),
                JsonValue::StringValue("running".into()).into(),
            ),
        ])
    }

    /// Clean up resources before unloading
    fn shutdown() {
        println!("  [GetTime Plugin] Shutting down gracefully");
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
                _ => Err(format!(
                    "Argument '{name}' at index {index} must be a string, got {:?}",
                    value
                )),
            }
        })
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
