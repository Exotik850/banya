use std::ops::Deref;

use crate::bindings::banya::controller::json::{Id, Value};

pub mod bindings {
    wit_bindgen::generate!({
        path: "./wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "banya_plugin::bindings",
        generate_all,
        ownership: Borrowing {
          duplicate_if_necessary: true
        } 
    });
}

impl<I: Deref<Target = Id>> From<I> for Value {
    fn from(value: I) -> Self {
        value.get()
    }
}

impl From<Value> for Id {
    fn from(value: Value) -> Self {
        bindings::banya::controller::json::put(value)
    }
}
