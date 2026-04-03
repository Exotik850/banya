use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value as JsonValue};
use wasmtime::AsContextMut;

use crate::PluginHost;

// --- Callable Reference ------------------------------------------------------

/// A callable target and capability pair.
///
/// Supports three equivalent JSON forms:
/// - `{"function": "math", "capability": "calculate"}`
/// - `{"plugin": "math", "capability": "calculate"}` (legacy alias)
/// - `{"call": "math.calculate"}` (ergonomic shorthand)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CallableRef {
    pub function: String,
    pub capability: String,
}

#[derive(Debug, Deserialize)]
struct CallableRefDe {
    #[serde(default)]
    call: Option<String>,
    #[serde(default, alias = "plugin", alias = "target", alias = "name")]
    function: Option<String>,
    #[serde(default)]
    capability: Option<String>,
}

impl<'de> Deserialize<'de> for CallableRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = CallableRefDe::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<CallableRefDe> for CallableRef {
    type Error = String;

    fn try_from(raw: CallableRefDe) -> Result<Self, Self::Error> {
        let parsed_call = raw
            .call
            .as_deref()
            .map(parse_call_shorthand)
            .transpose()?;

        let function = raw
            .function
            .or_else(|| parsed_call.as_ref().map(|(f, _)| f.clone()))
            .ok_or_else(|| {
                "Missing function reference. Use 'function'/'plugin' or shorthand 'call'."
                    .to_string()
            })?;

        let capability = raw
            .capability
            .or_else(|| parsed_call.as_ref().map(|(_, c)| c.clone()))
            .ok_or_else(|| {
                "Missing capability. Use 'capability' or shorthand 'call'.".to_string()
            })?;

        if let Some((call_function, call_capability)) = parsed_call {
            if call_function != function || call_capability != capability {
                return Err(
                    "Conflicting invocation fields: 'call' must match explicit 'function' and 'capability'."
                        .to_string(),
                );
            }
        }

        Ok(Self {
            function,
            capability,
        })
    }
}

fn parse_call_shorthand(value: &str) -> Result<(String, String), String> {
    for separator in ['.', ':', '/'] {
        if let Some((function, capability)) = value.rsplit_once(separator)
            && !function.is_empty()
            && !capability.is_empty()
        {
            return Ok((function.to_string(), capability.to_string()));
        }
    }

    Err(format!(
        "Invalid call shorthand '{value}'. Use 'function.capability', 'function:capability', or 'function/capability'."
    ))
}

// --- Invocation -------------------------------------------------------------

/// A single invocation of a capability exposed by a loaded or native function.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Invocation {
    #[serde(flatten)]
    pub callable: CallableRef,
    /// Arbitrary arguments passed to the capability.
    #[serde(flatten)]
    pub args: Map<String, JsonValue>,
}

/// Backwards-compatible type name used by external code.
pub type CapabilityCall = Invocation;

impl Invocation {
    #[must_use]
    pub fn function(&self) -> &str {
        &self.callable.function
    }

    #[must_use]
    pub fn capability(&self) -> &str {
        &self.callable.capability
    }

    fn invoke_json<S: AsContextMut<Data = PluginHost>>(&self, mut store: S) -> Result<JsonValue, String> {
        let args_json = {
            let host = store.as_context().data();
            interpolate_json(JsonValue::Object(self.args.clone()), host)?
        };

        let callable = {
            let host = store.as_context().data();
            host.resolve_callable(self.function())
        }
        .ok_or_else(|| {
            format!(
                "No loaded or native function found for invocation target: {}",
                self.function()
            )
        })?;

        callable.invoke_json(store.as_context_mut(), self.capability(), &args_json)
    }

    /// Execute this invocation and return the serialized JSON bytes.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(&self, store: S) -> Result<Vec<u8>, String> {
        let result = self.invoke_json(store)?;
        json_to_bytes(&result)
    }
}

// --- Condition --------------------------------------------------------------

/// A condition that gates whether a step executes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Condition {
    #[serde(flatten)]
    pub call: Invocation,
    /// If true, the condition result is negated.
    #[serde(default)]
    pub negate: bool,
}

impl Condition {
    /// Evaluate the condition invocation and convert the result to a boolean.
    pub fn evaluate<S: AsContextMut<Data = PluginHost>>(&self, store: S) -> Result<bool, String> {
        let result_value = self.call.invoke_json(store)?;

        let bool_result = match result_value {
            JsonValue::Bool(b) => b,
            JsonValue::Null => false,
            _ => true,
        };

        Ok(if self.negate { !bool_result } else { bool_result })
    }
}

// --- Step -------------------------------------------------------------------

/// A single step in an instruction pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Step {
    /// Optional condition. If present and false, this step is skipped.
    #[serde(rename = "if", alias = "when", skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// The invocation to execute.
    #[serde(flatten)]
    pub call: Invocation,
    /// Optional variable name to store the result for later interpolation.
    #[serde(alias = "as", skip_serializing_if = "Option::is_none")]
    pub store_as: Option<String>,
}

// --- Instruction ------------------------------------------------------------

/// A complete instruction: an ordered list of steps to execute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instruction {
    /// Optional name for identification and logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ordered list of steps.
    pub steps: Vec<Step>,
}

impl Instruction {
    /// Create a new instruction with the given name and steps.
    #[must_use]
    pub fn new(name: impl Into<Option<String>>, steps: Vec<Step>) -> Self {
        Self {
            name: name.into(),
            steps,
        }
    }

    /// Create an instruction with a single step.
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

    /// Validate that all invocation targets exist in the host.
    pub fn validate(&self, host: &PluginHost) -> Result<ValidatedInstruction, String> {
        let mut validated_steps = Vec::with_capacity(self.steps.len());

        for (i, step) in self.steps.iter().enumerate() {
            if let Some(condition) = &step.condition
                && host.resolve_callable(condition.call.function()).is_none()
            {
                return Err(format!(
                    "Step {}: No loaded or native function found for condition target: {}",
                    i,
                    condition.call.function()
                ));
            }

            if host.resolve_callable(step.call.function()).is_none() {
                return Err(format!(
                    "Step {}: No loaded or native function found for call target: {}",
                    i,
                    step.call.function()
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

// --- Validated Types --------------------------------------------------------

/// A step that has been validated and is ready for execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedStep {
    pub condition: Option<Condition>,
    pub call: CapabilityCall,
    pub store_as: Option<String>,
}

/// An instruction that has been validated and is ready for execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedInstruction {
    pub name: Option<String>,
    pub steps: Vec<ValidatedStep>,
}

impl ValidatedInstruction {
    /// Check if this instruction has any steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get the number of steps in this instruction.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Execute all steps in sequence and return the final JSON result as UTF-8.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(&self, mut store: S) -> Result<String, String> {
        let mut last_result = json_null_bytes();

        for step in &self.steps {
            if let Some(condition) = &step.condition {
                let condition_met = condition.evaluate(store.as_context_mut())?;
                if !condition_met {
                    continue;
                }
            }

            let result = step.call.execute(store.as_context_mut())?;
            last_result.clone_from(&result);

            // Always keep the most recent result available for interpolation.
            store
                .as_context_mut()
                .data_mut()
                .data_store
                .insert("last".to_string(), result.clone());

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
