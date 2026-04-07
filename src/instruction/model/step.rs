use serde::{Deserialize, Serialize};

use crate::{
    PluginHost,
    instruction::types::{Invalid, Valid},
};

use super::{
    condition::{Condition, ConditionDe},
    invocation::{Invocation, InvocationDe},
};

// --- Step -------------------------------------------------------------------

/// A single step in an instruction pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Step<S> {
    /// Optional condition. If present and false, this step is skipped.
    #[serde(rename = "if", alias = "when", skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition<S>>,

    /// The next steps to execute, if this step is successful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<Step<S>>>,

    /// The invocation to execute.
    #[serde(flatten)]
    pub call: Invocation<S>,
    /// Optional variable name to store the result for later interpolation,
    /// defaults to 'last' if not specified.
    ///
    /// This stores the raw JSON bytes of the result, which can be accessed by other steps or plugins via interpolation (e.g. `${variable_name}`).
    /// or the `get` function of the controller host interface.
    #[serde(alias = "as", skip_serializing_if = "Option::is_none")]
    pub store_as: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StepDe {
    #[serde(flatten)]
    call: InvocationDe,
    #[serde(rename = "if", alias = "when", skip_serializing_if = "Option::is_none")]
    condition: Option<ConditionDe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next: Option<Vec<StepDe>>,
    #[serde(alias = "as", skip_serializing_if = "Option::is_none")]
    store_as: Option<String>,
}

impl TryFrom<StepDe> for Step<Invalid> {
    type Error = String;

    fn try_from(raw: StepDe) -> Result<Self, Self::Error> {
        let call = Invocation::try_from(raw.call)?;
        let condition: Option<Condition<Invalid>> =
            raw.condition.map(Condition::try_from).transpose()?;
        let next = raw
            .next
            .map(|steps| {
                steps
                    .into_iter()
                    .map(Step::try_from)
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?;

        Ok(Self {
            call,
            condition,
            next,
            store_as: raw.store_as,
        })
    }
}

impl Step<Invalid> {
    pub fn validate(self, host: &PluginHost) -> Result<Step<Valid>, String> {
        let condition = if let Some(condition) = self.condition {
            Some(condition.validate(host)?)
        } else {
            None
        };

        let call = self.call.validate(host)?;

        let next = self
            .next
            .map(|steps| {
                steps
                    .into_iter()
                    .map(|step| step.validate(host))
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?;

        Ok(Step {
            condition,
            call,
            store_as: self.store_as,
            next,
        })
    }
}
