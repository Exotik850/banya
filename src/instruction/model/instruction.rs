use serde::{Deserialize, Deserializer, Serialize};
use wasmtime::AsContextMut;

use crate::{
    PluginHost,
    instruction::types::{Invalid, Valid},
};

use super::{
    interpolation::json_null_bytes,
    invocation::Invocation,
    step::{Step, StepDe},
};

// --- Instruction ------------------------------------------------------------

/// A complete instruction: an ordered list of steps to execute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Instruction<S> {
    /// Optional name for identification and logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ordered list of steps.
    #[serde(default)]
    pub steps: Vec<Step<S>>,
}

#[derive(Debug, Deserialize)]
struct InstructionDe {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default)]
    steps: Vec<StepDe>,
}

impl TryFrom<InstructionDe> for Instruction<Invalid> {
    type Error = String;

    fn try_from(raw: InstructionDe) -> Result<Self, Self::Error> {
        let steps = raw
            .steps
            .into_iter()
            .map(Step::try_from)
            .collect::<Result<Vec<_>, String>>()?;

        Ok(Self {
            name: raw.name,
            steps,
        })
    }
}

impl<'de> Deserialize<'de> for Instruction<Invalid> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = InstructionDe::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl<S> Instruction<S> {
    /// Create a new instruction with the given name and steps.
    #[must_use]
    pub fn new(name: impl Into<Option<String>>, steps: Vec<Step<S>>) -> Self {
        Self {
            name: name.into(),
            steps,
        }
    }

    /// Create an instruction with a single step.
    #[must_use]
    pub fn single(call: Invocation<S>) -> Self {
        Self {
            name: None,
            steps: vec![Step {
                condition: None,
                call,
                store_as: None,
                next: None,
            }],
        }
    }
}

impl Instruction<Invalid> {
    /// Validate that all invocation targets exist in the host.
    pub fn validate(self, host: &PluginHost) -> Result<Instruction<Valid>, String> {
        let validated_steps = self
            .steps
            .into_iter()
            .map(|step| step.validate(host))
            .collect::<Result<Vec<_>, String>>()?;

        Ok(Instruction {
            name: self.name,
            steps: validated_steps,
        })
    }
}

impl Instruction<Valid> {
    /// Execute all steps in sequence and return the final JSON result as UTF-8.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<String, String> {
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
