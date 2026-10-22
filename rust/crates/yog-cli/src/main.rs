//! `yog` — the Yog mod build tool.
//!
//! Mod authors never touch Cargo.toml.  They write `yog.toml`, call `yog build`,
//! and get a cross-platform `.yog` artifact.  The Cargo workspace is generated
//! in `.yog-build/` and is completely hidden from the author.
//!
//! Commands:
//!   yog new <name>   — scaffold a new mod project
//!   yog build        — compile + package the current mod
//!   yog setup        — check/install build dependencies
//!   yog doctor       — report build dependencies (read-only; `--json` for tools)
//!   yog check        — generate `.yog-build/` and `cargo check` the mod
//!   yog ui gen [..]  — generate `.yog-build/generated/**/*.g.rs` from `.yui` screens
//!   yog res check    — check `resources/**/*.yres`, generate their accessors
//!   yog help         — show usage

mod cli;

fn main() {
    // The library's warnings go to stderr: stdout is some commands' output (`yog schema`'s JSON).
    yog_logging::set_sink(|level, message| {
        let said = match level {
            yog_logging::Level::Info => "note",
            yog_logging::Level::Warn => "warning",
            yog_logging::Level::Error => "error",
        };
        eprintln!("{said}: {message}");
    });
    let args: Vec<String> = std::env::args().collect();
    cli::main(&args);
}
