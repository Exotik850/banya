use banya_plugin::bindings::{export, exports::banya::plugin::sensor::Guest};

export!(SensorTest);

pub struct SensorTest;

impl Guest for SensorTest {
    fn matches(data: String) -> bool {
        println!("Sensor received data: {}", data);
        data == "value"
    }
}
