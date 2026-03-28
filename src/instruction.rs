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
    #[serde(skip)]
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
    sensor: Option<NamedValue<Sensor>>,
    action: Option<NamedValue<Action>>,
}

impl Instruction {
    pub fn validate(self, host: &PluginHost) -> Result<ValidatedInstruction, String> {
        // Validate sensor
        if let Some(sensor) = &self.sensor {
            if !host.plugins.contains_key(&sensor.name) {
                return Err(format!("No plugin found for sensor: {}", sensor.name));
            }
        }

        // Validate action

        if let Some(action) = &self.action {
            if !host.plugins.contains_key(&action.name) {
                return Err(format!("No plugin found for action: {}", action.name));
            }
        }
        Ok(ValidatedInstruction {
            sensor: self.sensor,
            action: self.action,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ValidatedInstruction {
    sensor: Option<NamedValue<Sensor>>,
    action: Option<NamedValue<Action>>,
}

impl ValidatedInstruction {
    pub fn matches<S: AsContextMut<Data = PluginHost>>(&self, mut store: S) -> bool {
        let Some(sensor) = &self.sensor else {
            return true; // No sensor means always matches
        };

        let sensor_plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&sensor.name)
                .expect("Plugin should exist since it was validated")
                .clone()
        };
        let sensor_data =
            serde_json::to_string(&sensor.data).expect("Sensor data should be serializable");
        let sensor_result = sensor_plugin
            .execute(&mut store, sensor_data)
            .unwrap_or_else(|e| {
                panic!("Failed to execute sensor plugin '{}': {}", sensor.name, e);
            });
        match &sensor_result.trim().to_lowercase()[..] {
            "true" => true,
            "false" => false,
            _ => {
                panic!(
                    "Sensor plugin '{}' returned non-boolean result: {}",
                    sensor.name, sensor_result
                );
            }
        }
    }

    fn action<S: AsContextMut<Data = PluginHost>>(&self, store: S) -> Result<String, String> {
        let Some(action) = &self.action else {
            return Err("No action specified".to_string());
        };

        let action_plugin = {
            let host = store.as_context().data();
            host.plugins
                .get(&action.name)
                .ok_or_else(|| format!("No plugin found for action: {}", action.name))?
                .clone()
        };
        let action_data = serde_json::to_string(&action.data)
            .map_err(|e| format!("Failed to serialize action data: {}", e))?;
        action_plugin.execute(store, action_data)
    }

    pub fn execute<S: AsContextMut<Data = PluginHost>>(
        &self,
        mut store: S,
    ) -> Result<String, String> {
        // Execute sensor plugin
        if self.sensor.is_none() && self.action.is_none() {
            return Err("Instruction must have at least a sensor or an action".to_string());
        }

        if !self.matches(&mut store) {
            return Err("Sensor condition did not match, action will not be executed".to_string());
        }

        self.action(store)
    }
}
