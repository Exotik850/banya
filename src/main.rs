use std::path::{Path, PathBuf};

use banya::builtin::logical::{Compare, LogicalAnd, LogicalNot, LogicalOr, Math, StringOps};
use banya::{PluginHost, bindings::Plugin, instruction::Instruction};
use clap::Parser;
use wasmtime::{Config, Engine, Store, component::Linker};
use wasmtime_wasi::WasiCtx;

mod args;

fn main() {
    let args = args::Args::parse();

    // Set up WASI context and plugin host
    let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
    let mut state = PluginHost::new(wasi);

    // Register compile-time native functions
    register_builtin_functions(&mut state);

    // Configure engine with component model support
    let engine = Engine::new(Config::new().wasm_component_model(true)).unwrap();
    let mut store = Store::new(&engine, state);

    // Create a single unified linker for all plugins
    let linker = get_linker(&engine);

    {
        // Load all plugins from specified paths
        let mut load_plugin = |path: &Path| {
            let plugin = Plugin::from_file(path, &engine, &mut store, &linker)?;
            println!(
                "  Loaded plugin '{}' v{} with capabilities: {:?}",
                plugin.name(),
                plugin.version(),
                plugin.capabilities()
            );
            store.data_mut().push(plugin);
            Ok::<_, wasmtime::Error>(())
        };

        let mut paths = Vec::new();

        // Collect all .wasm files from directories and individual file paths
        for path in args.wasm_dir.iter().chain(args.wasm_file.iter()) {
            get_all_wasm_paths(path, &mut paths);
        }

        println!("Loading {} plugin(s)...", paths.len());
        paths.into_iter().for_each(|p| match load_plugin(&p) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to load plugin from {}: {}", p.display(), e);
            }
        });

        println!("Total plugins loaded: {}", store.data().len());
    }

    // Parse and execute instructions from JSON file
    let json_file = std::fs::File::open(&args.json_file).expect("Failed to open JSON file");
    let instruction: Instruction<_> =
        serde_json::from_reader(json_file).expect("Failed to parse JSON file");

    let validated = instruction
        .validate(store.data())
        .expect("Failed to validate instruction");

    let result = validated
        .execute(&mut store)
        .expect("Failed to run instructions");

    println!("Result: {result}");
}

/// Register all compile-time native functions with the host.
///
/// These functions are available alongside WASM plugins and can be invoked
/// using the same JSON instruction format. They have direct access to system
/// resources and can perform complex operations without WASM overhead.
fn register_builtin_functions(host: &mut PluginHost) {
    // Logical operations
    banya::register_native_functions!(
        host, LogicalAnd, LogicalOr, LogicalNot, Compare, StringOps, Math,
    );

    println!(
        "Registered {} native function(s): {:?}",
        host.len_native(),
        host.native_function_names().collect::<Vec<_>>()
    );
}

/// Recursively collect all .wasm files from a path
fn get_all_wasm_paths(path: impl AsRef<Path>, buffer: &mut Vec<PathBuf>) {
    let path = path.as_ref();
    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("wasm") {
        buffer.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            get_all_wasm_paths(path, buffer);
        }
    }
}

/// Create a single unified linker that supports WASI and the controller interface
fn get_linker(engine: &Engine) -> Linker<PluginHost> {
    let mut linker = Linker::new(engine);

    // Add WASI support for plugins
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();

    // Add the controller interface so plugins can call back into the host
    banya::bindings::host::banya::controller::controller::add_to_linker::<_, PluginHost>(
        &mut linker,
        |s: &mut PluginHost| s,
    )
    .unwrap();
    linker
}
