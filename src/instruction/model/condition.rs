use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;
use wasmtime::AsContextMut;

use crate::{
    PluginHost,
    instruction::types::{Invalid, Valid},
};

use super::invocation::{Invocation, InvocationDe};

// --- Condition --------------------------------------------------------------

/// A condition that gates whether a step executes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Condition<S> {
    #[serde(flatten)]
    pub call: Invocation<S>,
    /// If true, the condition result is negated.
    #[serde(default)]
    pub negate: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConditionDe {
    #[serde(flatten)]
    call: InvocationDe,
    #[serde(default)]
    negate: bool,
}

impl TryFrom<ConditionDe> for Condition<Invalid> {
    type Error = String;

    fn try_from(raw: ConditionDe) -> Result<Self, Self::Error> {
        let call = Invocation::try_from(raw.call)?;
        Ok(Self {
            call,
            negate: raw.negate,
        })
    }
}

impl<'de> Deserialize<'de> for Condition<Invalid> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ConditionDe::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl<S> Condition<S> {
    pub fn function(&self) -> &str {
        self.call.function()
    }

    pub fn capability(&self) -> &str {
        self.call.capability()
    }
}

impl Condition<Invalid> {
    pub fn validate(self, host: &PluginHost) -> Result<Condition<Valid>, String> {
        Ok(Condition {
            call: self.call.validate(host)?,
            negate: self.negate,
        })
    }
}

impl Condition<Valid> {
    /// Evaluate the condition invocation and convert the result to a boolean.
    pub fn evaluate<S: AsContextMut<Data = PluginHost>>(&self, store: S) -> Result<bool, String> {
        let result_value = self.call.invoke_json(store)?;

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
