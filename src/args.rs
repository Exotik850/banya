use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Args {
    #[clap(short)]
    pub json_file: PathBuf,

    /// Directories containing WebAssembly modules to load as plugins (e.g. sensor and action plugins)
    #[clap(short, long)]
    pub wasm_dir: Vec<PathBuf>,

    // path to additional wasm files that the main plugin may depend on (e.g. sensor and action plugins)
    #[clap(short, long)]
    pub wasm_file: Vec<PathBuf>,
}
