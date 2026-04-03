use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use wasmtime::AsContextMut;

use crate::PluginHost;

// ─── Capability Call ─────────────────────────────────────────────────────────

/// A single invocation of a plugin capability.
///
/// This is the atomic unit of work in an instruction. It references a plugin by
/// name, specifies which capability to invoke, and carries arbitrary arguments
/// that are flattened into the JSON for ergonomic configuration.
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "echo",
///   "capability": "action",
///   "message": "Hello, world!"
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityCall {
    /// The name of the plugin to invoke
    pub plugin: String,
    /// The capability to invoke on the plugin (e.g., "sensor", "action", "transform")
    pub capability: String,
    /// Arbitrary arguments passed to the capability, flattened into the JSON object
    /// String values can use `${name}` to interpolate stored step results
    #[serde(flatten)]
    pub args: Map<String, JsonValue>,
}

// ─── Condition ───────────────────────────────────────────────────────────────

/// A condition that gates whether a step executes.
///
/// Evaluates a plugin capability (typically a "sensor") and uses the boolean
/// result to determine if the associated step should run. Supports optional
/// negation for inverted logic.
///
/// # Example (JSON)
/// ```json
/// {
///   "if": {
///     "plugin": "temperature-sensor",
///     "capability": "sensor",
///     "threshold": 75
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Condition {
    /// The plugin to evaluate
    pub plugin: String,
    /// The capability to invoke (typically "sensor")
    pub capability: String,
    /// Arguments for the condition evaluation
    #[serde(flatten)]
    pub args: Map<String, JsonValue>,
    /// If true, the condition result is negated
    #[serde(default)]
    pub negate: bool,
}

// ─── Step ────────────────────────────────────────────────────────────────────

/// A single step in an instruction pipeline.
///
/// Each step optionally checks a condition before executing a capability call.
/// The result can be stored under a named variable for use by subsequent steps.
///
/// # Example (JSON)
/// ```json
/// {
///   "if": {
///     "plugin": "motion-sensor",
///     "capability": "sensor"
///   },
///   "plugin": "camera",
///   "capability": "action",
///   "mode": "capture",
///   "store_as": "snapshot"
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Step {
    /// Optional condition — if present and evaluates to false, this step is skipped
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// The capability call to execute
    #[serde(flatten)]
    pub call: CapabilityCall,
    /// Optional variable name to store the result in for later steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_as: Option<String>,
}

// ─── Instruction ─────────────────────────────────────────────────────────────

/// A complete instruction — an ordered pipeline of steps to execute.
///
/// Instructions are the primary configuration unit that end-users write as JSON
/// files. Each instruction chains together plugin capability calls, with
/// optional conditions and result passing between steps.
///
/// # Example (JSON)
/// ```json
/// {
///   "name": "morning-routine",
///   "steps": [
///     {
///       "plugin": "time-sensor",
///       "capability": "sensor",
///       "check": "is_morning",
///       "store_as": "is_morning"
///     },
///     {
///       "if": {
///         "plugin": "time-sensor",
///         "capability": "sensor",
///         "check": "is_morning"
///       },
///       "plugin": "lights",
///       "capability": "action",
///       "brightness": 80
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instruction {
    /// Optional name for this instruction (useful for identification and logging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ordered list of steps to execute in sequence
    pub steps: Vec<Step>,
}

impl Instruction {
    /// Create a new instruction with the given name and steps
    #[must_use]
    pub fn new(name: impl Into<Option<String>>, steps: Vec<Step>) -> Self {
        Self {
            name: name.into(),
            steps,
        }
    }

    /// Create an instruction with a single step (convenience constructor)
    #[must_use]
    pub fn single(call: CapabilityCall) -> Self {
        Self {
            name: None,
            steps: vec![Step {
                condition: None,
                call,
                store_as: None,
            }],
        }
    }

    /// Validate that all plugin references in this instruction exist in the host
    pub fn validate(&self, host: &PluginHost) -> Result<ValidatedInstruction, String> {
        let mut validated_steps = Vec::with_capacity(self.steps.len());

        for (i, step) in self.steps.iter().enumerate() {
            // Validate condition plugin exists
            if let Some(condition) = &step.condition
                && !host.plugins.contains_key(&condition.plugin) {
                    return Err(format!(
                        "Step {}: No plugin found for condition: {}",
                        i, condition.plugin
                    ));
                }

            // Validate call plugin exists
            if !host.plugins.contains_key(&step.call.plugin) {
                return Err(format!(
                    "Step {}: No plugin found for call: {}",
                    i, step.call.plugin
                ));
            }

            validated_steps.push(ValidatedStep {
                condition: step.condition.clone(),
                call: step.call.clone(),
                store_as: step.store_as.clone(),
            });
        }

        Ok(ValidatedInstruction {
            name: self.name.clone(),
            steps: validated_steps,
        })
    }
}

// ─── Validated Types ─────────────────────────────────────────────────────────

/// A step that has been validated and is ready for execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedStep {
    /// Optional condition — if present and evaluates to false, this step is skipped
    pub condition: Option<Condition>,
    /// The capability call to execute
    pub call: CapabilityCall,
    /// Optional variable name to store the result in for later steps
    pub store_as: Option<String>,
}

/// An instruction that has been validated and is ready for execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedInstruction {
    /// Optional name for this instruction
    pub name: Option<String>,
    /// Validated steps ready for execution
    pub steps: Vec<ValidatedStep>,
}

impl ValidatedInstruction {
    /// Check if this instruction has any steps
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get the number of steps in this instruction
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Execute all steps in sequence, returning the final result.
    ///
    /// Each step is executed in order. If a step has a condition, it is evaluated
    /// first — if the condition is false, the step is skipped. Results can be
    /// stored in the host's JSON store for use by subsequent steps.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<String, String> {
        let mut last_result = json_null_bytes();

        for step in &self.steps {
            // Evaluate condition if present
            if let Some(condition) = &step.condition {
                let condition_met = condition.evaluate(&mut store)?;
                if !condition_met {
                    // Skip this step — condition not met
                    continue;
                }
            }

            // Execute the capability call
            let result = step.call.execute(&mut store)?;
            last_result.clone_from(&result);

            // Store result if requested
            if let Some(name) = &step.store_as {
                store
                    .as_context_mut()
                    .data_mut()
                    .data_store
                    .insert(name.clone(), result);
            }
        }

        String::from_utf8(last_result).map_err(|e| format!("Result is not valid UTF-8 JSON: {e}"))
    }
}

impl Condition {
    /// Evaluate the condition by invoking the plugin capability.
    ///
    /// Returns `true` if the capability returns a boolean `true`, or if the
    /// result is truthy (non-null, non-false). The `negate` flag inverts the result.
    pub fn evaluate<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<bool, String> {
        let plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&self.plugin)
                .ok_or_else(|| format!("Plugin '{}' not found for condition", self.plugin))?
                .clone()
        };

        let args_json = {
            let host = store.as_context().data();
            interpolate_json(JsonValue::Object(self.args.clone()), host)?
        };
        let args_bytes = json_to_bytes(&args_json)?;

        let result = plugin.invoke(&mut store, &self.capability, &args_bytes)?;
        let result_value = match result {
            Some(bytes) => json_from_bytes(&bytes)?,
            None => JsonValue::Null,
        };

        let bool_result = match result_value {
            JsonValue::Bool(b) => b,
            JsonValue::Null => false,
            _ => true,
        };

        Ok(if self.negate {
            !bool_result
        } else {
            bool_result
        })
    }
}

impl CapabilityCall {
    /// Execute this capability call against the plugin host.
    ///
    /// Serializes the arguments, invokes the plugin capability, and returns the result.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<Vec<u8>, String> {
        let plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&self.plugin)
                .ok_or_else(|| format!("Plugin '{}' not found for capability call", self.plugin))?
                .clone()
        };

        let args_json = {
            let host = store.as_context().data();
            interpolate_json(JsonValue::Object(self.args.clone()), host)?
        };
        let args_bytes = json_to_bytes(&args_json)?;

        let result = plugin.invoke(&mut store, &self.capability, &args_bytes)?;

        Ok(result.unwrap_or_else(json_null_bytes))
    }
}

fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| format!("Failed to serialize JSON: {e}"))
}

fn json_from_bytes(bytes: &[u8]) -> Result<JsonValue, String> {
    if bytes.is_empty() {
        return Ok(JsonValue::Null);
    }
    serde_json::from_slice(bytes).map_err(|e| format!("Failed to parse JSON bytes: {e}"))
}

fn json_null_bytes() -> Vec<u8> {
    b"null".to_vec()
}

fn interpolate_json(value: JsonValue, host: &PluginHost) -> Result<JsonValue, String> {
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

// interpolates string in place
fn interpolate_string(mut value: String, host: &PluginHost) -> Result<JsonValue, String> {
    let mut cursor = 0;

    while let Some(start_offset) = value[cursor..].find("${") {
        let start = cursor + start_offset;
        // out.push_str(&value[cursor..start]);

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
        cursor = name_end + 1;
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
    if let Some(bytes) = host.data_store.get(name) { json_from_bytes(bytes) } else {
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
