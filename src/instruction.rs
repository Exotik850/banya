use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wasmtime::AsContextMut;

use crate::PluginHost;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sensor;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub struct NamedValue<T> {
    name: String,
    #[serde(flatten)]
    data: Map<String, Value>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Clone for NamedValue<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            data: self.data.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Instruction {
    sensor: NamedValue<Sensor>,
    action: NamedValue<Action>,
}

impl Instruction {
    pub fn validate(self, host: &PluginHost) -> Result<ValidatedInstruction, String> {
        // Validate sensor
        if !host.plugins.contains_key(&self.sensor.name) {
            return Err(format!("No plugin found for sensor: {}", self.sensor.name));
        }
        // Validate action
        if !host.plugins.contains_key(&self.action.name) {
            return Err(format!("No plugin found for action: {}", self.action.name));
        }
        Ok(ValidatedInstruction {
            sensor: ValidatedValue { value: self.sensor },
            action: ValidatedValue { value: self.action },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedValue<T> {
    value: NamedValue<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedInstruction {
    sensor: ValidatedValue<Sensor>,
    action: ValidatedValue<Action>,
}

impl ValidatedInstruction {
    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<String, String> {
        // Execute sensor plugin
        let sensor_plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&self.sensor.value.name)
                .ok_or_else(|| format!("No plugin found for sensor: {}", self.sensor.value.name))?
                .clone()
        };
        let sensor_data = serde_json::to_string(&self.sensor.value.data)
            .map_err(|e| format!("Failed to serialize sensor data: {}", e))?;
        let sensor_result = sensor_plugin.execute(&mut store, sensor_data)?;

        if !serde_json::from_str::<bool>(&sensor_result).unwrap_or(true) {
            return Err(format!(
                "Sensor plugin '{}' returned false, skipping action execution",
                self.sensor.value.name
            ));
        }

        let action_data = serde_json::to_string(&self.action.value.data)
            .map_err(|e| format!("Failed to serialize action data: {}", e))?;
        // Execute action plugin with sensor result
        let action_plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&self.action.value.name)
                .ok_or_else(|| format!("No plugin found for action: {}", self.action.value.name))?
                .clone()
        };
        action_plugin.execute(store, action_data)
    }
}
