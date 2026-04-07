use std::path::{Path, PathBuf};
use std::time::Duration;

use banya::bindings::Plugin;
use banya::builtin::logical::{Compare, LogicalAnd, LogicalNot, LogicalOr, Math, StringOps};
use banya::instruction::Instruction;
use banya::PluginHost;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use serde_json::{Value as JsonValue, json};
use wasmtime::{Config, Engine, Store, component::Linker};
use wasmtime_wasi::WasiCtx;

const WASM_INTEROP_CYCLES: usize = 250;
const MIXED_INTEROP_CYCLES: usize = 120;
const MIXED_NATIVE_ROUNDS: usize = 1_000;

struct BenchmarkRuntime {
    store: Store<PluginHost>,
}

fn benchmark_runtime() -> BenchmarkRuntime {
    let engine = Engine::new(Config::new().wasm_component_model(true))
        .expect("Failed to initialize Wasmtime engine for benchmark runtime");
    let mut store = Store::new(&engine, new_host());
    let linker = linker(&engine);

    for wasm_file in ["echo.wasm", "get_time.wasm", "print_time.wasm"] {
        let path = wasm_path(wasm_file);
        assert!(
            path.exists(),
            "Missing benchmark dependency '{}'. Build plugin examples and place wasm artifacts in ./wasms.",
            path.display()
        );

        let plugin = Plugin::from_file(&path, &engine, &mut store, &linker)
            .unwrap_or_else(|e| panic!("Failed to load plugin '{}': {e}", path.display()));
        store.data_mut().push(plugin);
    }

    BenchmarkRuntime { store }
}

fn new_host() -> PluginHost {
    let wasi = WasiCtx::builder().build();
    let mut host = PluginHost::new(wasi);

    banya::register_native_functions!(
        host, LogicalAnd, LogicalOr, LogicalNot, Compare, StringOps, Math,
    );

    host
}

fn linker(engine: &Engine) -> Linker<PluginHost> {
    let mut linker = Linker::new(engine);

    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .expect("Failed to add WASI to benchmark linker");

    banya::bindings::host::banya::controller::controller::add_to_linker::<_, PluginHost>(
        &mut linker,
        |state: &mut PluginHost| state,
    )
    .expect("Failed to add controller host interface to benchmark linker");

    linker
}

fn wasm_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("wasms")
        .join(file_name)
}

fn build_wasm_interop_payload(cycles: usize) -> JsonValue {
    let mut steps = Vec::with_capacity(cycles * 5 + 2);

    steps.push(json!({
        "call": "get-time.sensor",
        "format": "%Y-%m-%d %H:%M:%S%.3f",
        "as": "bench_start"
    }));

    for cycle in 1..=cycles {
        steps.push(json!({
            "call": "get-time.sensor",
            "format": "%H:%M:%S%.3f"
        }));

        steps.push(json!({
            "call": "print-time.action",
            "time": "${last}",
            "message": format!("interop cycle {cycle}")
        }));

        steps.push(json!({
            "call": "echo.action",
            "data": format!("cycle-{cycle} => ${{last}}")
        }));

        steps.push(json!({
            "call": "string-ops.evaluate",
            "operation": "contains",
            "value": "${last}",
            "pattern": "Echo:"
        }));

        steps.push(json!({
            "call": "logical-and.evaluate",
            "conditions": [true, "${last}", true]
        }));
    }

    steps.push(json!({
        "call": "get-time.sensor",
        "format": "%Y-%m-%d %H:%M:%S%.3f",
        "as": "bench_end"
    }));

    steps.push(json!({
        "call": "echo.action",
        "data": "criterion wasm interop benchmark complete | start=${bench_start} | end=${bench_end} | final=${last}"
    }));

    json!({
        "name": "criterion-wasm-plugin-interop",
        "steps": steps
    })
}

fn build_mixed_payload(interop_cycles: usize, native_rounds: usize) -> JsonValue {
    let mut steps = Vec::with_capacity(interop_cycles * 5 + native_rounds * 4 + 2);

    steps.push(json!({
        "call": "get-time.sensor",
        "format": "%Y-%m-%d %H:%M:%S%.3f",
        "as": "bench_start"
    }));

    for cycle in 1..=interop_cycles {
        steps.push(json!({
            "call": "get-time.sensor",
            "format": "%H:%M:%S%.3f"
        }));

        steps.push(json!({
            "call": "print-time.action",
            "time": "${last}",
            "message": format!("mixed interop cycle {cycle}")
        }));

        steps.push(json!({
            "call": "echo.action",
            "data": format!("mixed-cycle-{cycle} => ${{last}}")
        }));

        steps.push(json!({
            "call": "string-ops.evaluate",
            "operation": "contains",
            "value": "${last}",
            "pattern": "Echo:"
        }));

        steps.push(json!({
            "call": "logical-not.evaluate",
            "value": false
        }));
    }

    for round in 1..=native_rounds {
        let base = (round as i64) * 3;

        steps.push(json!({
            "call": "math.calculate",
            "operation": "add",
            "values": [base, base + 1, base + 2, base + 3, base + 4]
        }));

        steps.push(json!({
            "call": "compare.evaluate",
            "operator": "gt",
            "left": "${last}",
            "right": base * 2
        }));

        steps.push(json!({
            "call": "logical-and.evaluate",
            "conditions": [true, "${last}", true, true]
        }));

        steps.push(json!({
            "call": "string-ops.transform",
            "operation": "concat",
            "strings": ["native-round-", round.to_string(), "-status-", "${last}"]
        }));
    }

    steps.push(json!({
        "call": "get-time.sensor",
        "format": "%Y-%m-%d %H:%M:%S%.3f",
        "as": "bench_end"
    }));

    steps.push(json!({
        "call": "echo.action",
        "data": "criterion mixed benchmark complete | start=${bench_start} | end=${bench_end} | final=${last}"
    }));

    json!({
        "name": "criterion-mixed-plugin-native-interop",
        "steps": steps
    })
}

fn instruction_runtime_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("banya-intensive-runtime");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(12));
    group.sample_size(15);

    let mut wasm_runtime = benchmark_runtime();
    let wasm_payload = build_wasm_interop_payload(WASM_INTEROP_CYCLES);
    let wasm_instruction: Instruction<_> = serde_json::from_value(wasm_payload)
        .expect("Failed to parse WASM interop benchmark payload");
    let wasm_validated = wasm_instruction
        .validate(wasm_runtime.store.data())
        .expect("Failed to validate WASM interop benchmark payload");

    let wasm_calls = (WASM_INTEROP_CYCLES * 5 + 2) as u64;
    group.throughput(Throughput::Elements(wasm_calls));
    group.bench_function(
        BenchmarkId::new("wasm-plugin-interop-steps", wasm_calls),
        |b| {
            b.iter(|| {
                let result = wasm_validated
                    .execute(&mut wasm_runtime.store)
                    .expect("WASM interop benchmark execution failed");
                black_box(result);
            });
        },
    );

    let mut mixed_runtime = benchmark_runtime();
    let mixed_payload = build_mixed_payload(MIXED_INTEROP_CYCLES, MIXED_NATIVE_ROUNDS);
    let mixed_instruction: Instruction<_> = serde_json::from_value(mixed_payload)
        .expect("Failed to parse mixed benchmark payload");
    let mixed_validated = mixed_instruction
        .validate(mixed_runtime.store.data())
        .expect("Failed to validate mixed benchmark payload");

    let mixed_calls = (MIXED_INTEROP_CYCLES * 5 + MIXED_NATIVE_ROUNDS * 4 + 2) as u64;
    group.throughput(Throughput::Elements(mixed_calls));
    group.bench_function(
        BenchmarkId::new("mixed-plugin-native-steps", mixed_calls),
        |b| {
            b.iter(|| {
                let result = mixed_validated
                    .execute(&mut mixed_runtime.store)
                    .expect("Mixed benchmark execution failed");
                black_box(result);
            });
        },
    );

    group.finish();
}

criterion_group!(benches, instruction_runtime_benchmarks);
criterion_main!(benches);
