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
