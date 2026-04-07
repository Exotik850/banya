use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value as JsonValue};
use wasmtime::AsContextMut;

use crate::{
    PluginHost,
    instruction::types::{Invalid, Valid},
};

use super::{
    callable::{CallableRef, CallableRefDe},
    interpolation::{interpolate_json, json_to_bytes},
};

// --- Invocation -------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub(super) struct InvocationDe {
    #[serde(flatten)]
    callable: CallableRefDe,
    #[serde(flatten)]
    args: Map<String, JsonValue>,
}

impl TryFrom<InvocationDe> for Invocation<Invalid> {
    type Error = String;

    fn try_from(raw: InvocationDe) -> Result<Self, Self::Error> {
        let callable = CallableRef::try_from(raw.callable)?;
        Ok(Self {
            callable,
            args: raw.args,
        })
    }
}

/// A single invocation of a capability exposed by a loaded or native function.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Invocation<S> {
    #[serde(flatten)]
    pub callable: CallableRef<S>,
    /// Arbitrary arguments passed to the capability.
    #[serde(flatten)]
    pub args: Map<String, JsonValue>,
}

impl<'de> Deserialize<'de> for Invocation<Invalid> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = InvocationDe::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl<S> Invocation<S> {
    #[must_use]
    pub fn function(&self) -> &str {
        &self.callable.function
    }

    #[must_use]
    pub fn capability(&self) -> &str {
        &self.callable.capability
    }
}

impl Invocation<Invalid> {
    pub fn validate(self, host: &PluginHost) -> Result<Invocation<Valid>, String> {
        if host.resolve_callable(self.function()).is_none() {
            return Err(format!(
                "No loaded or native function found for invocation target: {}",
                self.function()
            ));
        }

        Ok(Invocation {
            callable: CallableRef {
                function: self.callable.function,
                capability: self.callable.capability,
                _marker: std::marker::PhantomData,
            },
            args: self.args,
        })
    }
}

impl Invocation<Valid> {
    pub(super) fn invoke_json<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<JsonValue, String> {
        let args_json = {
            let host = store.as_context().data();
            interpolate_json(JsonValue::Object(self.args.clone()), host)?
        };

        let callable = {
            let host = store.as_context().data();
            host.resolve_callable(self.function())
        }
        .expect("Validated Invocation should have callable target");

        callable.invoke_json(store.as_context_mut(), self.capability(), &args_json)
    }

    /// Execute this invocation and return the serialized JSON bytes.
    pub fn execute<S: AsContextMut<Data = PluginHost>>(&self, store: S) -> Result<Vec<u8>, String> {
        let result = self.invoke_json(store)?;
        json_to_bytes(&result)
    }
}
