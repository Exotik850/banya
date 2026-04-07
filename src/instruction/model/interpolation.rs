use serde_json::{Map, Value as JsonValue};

use crate::PluginHost;

pub(super) fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| format!("Failed to serialize JSON: {e}"))
}

fn json_from_bytes(bytes: &[u8]) -> Result<JsonValue, String> {
    if bytes.is_empty() {
        return Ok(JsonValue::Null);
    }
    serde_json::from_slice(bytes).map_err(|e| format!("Failed to parse JSON bytes: {e}"))
}

pub(super) fn json_null_bytes() -> Vec<u8> {
    b"null".to_vec()
}

pub(super) fn interpolate_json(value: JsonValue, host: &PluginHost) -> Result<JsonValue, String> {
    match value {
        JsonValue::String(value) => interpolate_string(value, host),
        JsonValue::Array(values) => {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                out.push(interpolate_json(value, host)?);
            }
            Ok(JsonValue::Array(out))
        }
        JsonValue::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (key, value) in map {
                out.insert(key, interpolate_json(value, host)?);
            }
            Ok(JsonValue::Object(out))
        }
        other => Ok(other),
    }
}

fn interpolate_string(mut value: String, host: &PluginHost) -> Result<JsonValue, String> {
    let mut cursor = 0;

    while let Some(start_offset) = value[cursor..].find("${") {
        let start = cursor + start_offset;

        let name_start = start + 2;
        let end_offset = value[name_start..]
            .find('}')
            .ok_or_else(|| format!("Unclosed interpolation in '{value}'"))?;
        let name_end = name_start + end_offset;
        let name = &value[name_start..name_end];

        validate_variable_name(name)?;
        let resolved = lookup_variable(name, host)?;

        if start == 0 && name_end + 1 == value.len() {
            return Ok(resolved);
        }

        let resolved_str = json_value_to_string(&resolved)?;
        value.replace_range(start..=name_end, &resolved_str);
        cursor = start + resolved_str.len();
        if cursor > value.len() {
            break;
        }
    }

    Ok(JsonValue::String(value))
}

fn validate_variable_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Interpolation variable name cannot be empty".to_string());
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Interpolation variable '{name}' contains invalid characters (allowed: ASCII letters, digits, '_' or '-')"
        ));
    }

    Ok(())
}

fn lookup_variable(name: &str, host: &PluginHost) -> Result<JsonValue, String> {
    if let Some(bytes) = host.data_store.get(name) {
        json_from_bytes(bytes)
    } else {
        let mut keys: Vec<&String> = host.data_store.keys().collect();
        keys.sort();
        let available = if keys.is_empty() {
            "none".to_string()
        } else {
            keys.iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        Err(format!(
            "Interpolation variable '{name}' not found. Available variables: {available}"
        ))
    }
}

fn json_value_to_string(value: &JsonValue) -> Result<String, String> {
    match value {
        JsonValue::String(value) => Ok(value.clone()),
        JsonValue::Null => Ok("null".to_string()),
        other => serde_json::to_string(other)
            .map_err(|e| format!("Failed to serialize interpolation value: {e}")),
    }
}
