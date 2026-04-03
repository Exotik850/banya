use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use wasmtime::AsContextMut;

use crate::PluginHost;
use crate::bindings::host;

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
            if let Some(condition) = &step.condition {
                if !host.plugins.contains_key(&condition.plugin) {
                    return Err(format!(
                        "Step {}: No plugin found for condition: {}",
                        i, condition.plugin
                    ));
                }
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
        let mut last_result = host::Value::Null;

        for (_i, step) in self.steps.iter().enumerate() {
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
            last_result = result.clone();

            // Store result if requested
            if let Some(name) = &step.store_as {
                let id = store.as_context_mut().data_mut().next_id();
                store
                    .as_context_mut()
                    .data_mut()
                    .json_store
                    .insert(id, result);
                store
                    .as_context_mut()
                    .data_mut()
                    .json_map
                    .insert(name.clone(), id);
            }
        }

        // Convert final result to JSON string
        let host = store.as_context().data();
        let json_result = last_result.to_json(host);
        serde_json::to_string(&json_result).map_err(|e| format!("Failed to serialize result: {e}"))
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

        // Serialize condition args into the JSON store and get resource ID
        let args_json = serde_json::Value::Object(self.args.clone());
        let args_id = Self::store_value(&mut store, args_json)?;

        // Invoke the capability
        let result = plugin.invoke(&mut store, &self.capability, &[args_id])?;

        // Parse the result as a boolean
        let result = result.ok_or_else(|| "Condition returned no result".to_string())?;
        let value = {
            let host = store.as_context().data();
            host.json_store
                .get(&result.rep())
                .cloned()
                .unwrap_or(host::Value::Null)
        };

        let bool_result = match value {
            host::Value::BoolValue(b) => b,
            host::Value::Null => false,
            _ => true, // Non-boolean, non-null results are considered truthy
        };

        Ok(if self.negate {
            !bool_result
        } else {
            bool_result
        })
    }

    /// Helper to store a JSON value in the host and return its resource ID
    fn store_value<S: AsContextMut<Data = PluginHost>>(
        store: &mut S,
        json: serde_json::Value,
    ) -> Result<wasmtime::component::Resource<host::banya::controller::json::Id>, String> {
        let value = host::Value::from_json(json, store.as_context_mut().data_mut());
        let id = store.as_context_mut().data_mut().next_id();
        store
            .as_context_mut()
            .data_mut()
            .json_store
            .insert(id, value);
        Ok(wasmtime::component::Resource::new_own(id))
    }
}

impl CapabilityCall {
    /// Execute this capability call against the plugin host.
    ///
    /// Serializes the arguments, invokes the plugin capability, and returns the result.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<host::Value, String> {
        let plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&self.plugin)
                .ok_or_else(|| format!("Plugin '{}' not found for capability call", self.plugin))?
                .clone()
        };

        // Serialize args into the JSON store and get resource ID
        let args_json = serde_json::Value::Object(self.args.clone());
        let args_id = Self::store_value(&mut store, args_json)?;

        // Invoke the capability
        let result = plugin.invoke(&mut store, &self.capability, &[args_id])?;

        // Resolve the result value
        Ok(match result {
            Some(id) => {
                let host = store.as_context().data();
                host.json_store
                    .get(&id.rep())
                    .cloned()
                    .unwrap_or(host::Value::Null)
            }
            None => host::Value::Null,
        })
    }

    /// Helper to store a JSON value in the host and return its resource ID
    fn store_value<S: AsContextMut<Data = PluginHost>>(
        store: &mut S,
        json: serde_json::Value,
    ) -> Result<wasmtime::component::Resource<host::banya::controller::json::Id>, String> {
        let value = host::Value::from_json(json, store.as_context_mut().data_mut());
        let id = store.as_context_mut().data_mut().next_id();
        store
            .as_context_mut()
            .data_mut()
            .json_store
            .insert(id, value);
        Ok(wasmtime::component::Resource::new_own(id))
    }
}
