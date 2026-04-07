use serde::{Deserialize, Deserializer, Serialize};

use crate::instruction::types::Invalid;

/// A callable target and capability pair.
///
/// Supports three equivalent JSON forms:
/// - `{"function": "math", "capability": "calculate"}`
/// - `{"plugin": "math", "capability": "calculate"}` (legacy alias)
/// - `{"call": "math.calculate"}` (ergonomic shorthand)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CallableRef<S> {
    pub function: String,
    pub capability: String,
    #[serde(skip)]
    pub(super) _marker: std::marker::PhantomData<S>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CallableRefDe {
    #[serde(default)]
    call: Option<String>,
    #[serde(default, alias = "plugin", alias = "target", alias = "name")]
    function: Option<String>,
    #[serde(default)]
    capability: Option<String>,
}

impl<'de> Deserialize<'de> for CallableRef<Invalid> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = CallableRefDe::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<CallableRefDe> for CallableRef<Invalid> {
    type Error = String;

    fn try_from(raw: CallableRefDe) -> Result<Self, Self::Error> {
        let parsed_call = raw.call.as_deref().map(parse_call_shorthand).transpose()?;

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
            _marker: std::marker::PhantomData,
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
