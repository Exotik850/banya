use std::path::{Path, PathBuf};

use banya::{PluginHost, bindings::Plugin, instruction::Instruction};
use clap::Parser;
use wasmtime::{Config, Engine, Store, component::Linker};
use wasmtime_wasi::WasiCtx;

mod args;
 
fn main() {
    let args = args::Args::parse();

    let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
    let state = PluginHost::new(wasi);
    let engine = Engine::new(Config::new().wasm_component_model(true)).unwrap();
    let mut store = Store::new(&engine, state);

    let (sensor_linker, action_linker) = get_linkers(&engine);

    {
        let mut load_plugin = |path: &Path| {
            let plugin =
                Plugin::from_file(path, &engine, &mut store, &sensor_linker, &action_linker)?;
            store.data_mut().push(plugin);
            Ok::<_, wasmtime::Error>(())
        };

        let mut paths = Vec::new();

        for path in args.wasm_dir.iter().chain(args.wasm_file.iter()) {
            get_all_wasm_paths(path, &mut paths);
        }

        paths.into_iter().for_each(|p| match load_plugin(&p) {
            Ok(()) => println!("Loaded plugin from {}", p.display()),
            Err(e) => {
                eprintln!("Failed to load plugin from {}: {}", p.display(), e);
            }
        });
    }
    let json_file = std::fs::File::open(&args.json_file).expect("Failed to open JSON file");
    let instruction: Instruction =
        serde_json::from_reader(json_file).expect("Failed to parse JSON file");

    let validated = instruction
        .validate(store.data())
        .expect("Failed to validate instruction");
    validated
        .execute(&mut store)
        .expect("Failed to run instructions");
}

fn get_all_wasm_paths(path: impl AsRef<Path>, buffer: &mut Vec<PathBuf>) {
    if path.as_ref().is_file() && path.as_ref().extension().and_then(|e| e.to_str()) == Some("wasm")
    {
        buffer.push(path.as_ref().to_path_buf());
    } else if path.as_ref().is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                buffer.push(path);
            } else {
                get_all_wasm_paths(path, buffer);
            }
        }
    }
}

fn get_linkers(engine: &Engine) -> (Linker<PluginHost>, Linker<PluginHost>) {
    let mut sensor_linker = Linker::new(engine);
    let mut action_linker = Linker::new(engine);

    wasmtime_wasi::p2::add_to_linker_sync(&mut sensor_linker).unwrap();
    wasmtime_wasi::p2::add_to_linker_sync(&mut action_linker).unwrap();

    banya::bindings::action::banya::controller::controller::add_to_linker::<_, PluginHost>(
        &mut action_linker,
        |s: &mut PluginHost| s,
    )
    .unwrap();
    banya::bindings::sensor::banya::controller::controller::add_to_linker::<_, PluginHost>(
        &mut sensor_linker,
        |s: &mut PluginHost| s,
    )
    .unwrap();
    (sensor_linker, action_linker)
}
