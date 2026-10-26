//! The crate root `yog build` writes for a mod without Rust of its own
//! (`src/lib.rs`): its code is Yog Script, and the native it still has
//! registers what the content files say.

use std::path::Path;

use yog_codegen::{Header, Regenerate};
use yog_codegen_rust::Rust;

use crate::project::manifest::{RUST_LIB, SOURCES_DIR};

/// The generated root's file, under `obj/generated/`.
const FILE: &str = "mod.g.rs";

/// The mod's crate root, as the generated `Cargo.toml` (in `obj/`) names
/// it: its own `src/lib.rs`, or the generated one.
pub fn lib_path(root: &Path) -> String {
    if has_rust(root) {
        format!("../{SOURCES_DIR}/{RUST_LIB}")
    } else {
        format!("{}/{FILE}", generated_dir())
    }
}

/// Whether the mod at `root` has Rust of its own.
pub fn has_rust(root: &Path) -> bool {
    root.join(SOURCES_DIR).join(RUST_LIB).is_file()
}

/// Writes the generated root when the mod has no `src/lib.rs` (only when
/// it changed); `content`: whether the build generated content code.
pub fn write(root: &Path, content: bool) -> Result<(), String> {
    if has_rust(root) {
        return Ok(());
    }
    let file = root
        .join(yog_ids::project::OBJ)
        .join(generated_dir())
        .join(FILE);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    crate::write_if_changed(&file, source(content))
}

/// `generated`: [`yog_ids::project::GENERATED`] under `obj/`.
fn generated_dir() -> &'static str {
    let obj = yog_ids::project::OBJ;
    yog_ids::project::GENERATED
        .strip_prefix(obj)
        .map_or(yog_ids::project::GENERATED, |p| {
            p.trim_start_matches(yog_ids::sep::PATH)
        })
}

/// The root's code: the mod, registering nothing itself — its content
/// (`yog_api::content!()`) registers when there is any.
fn source(content: bool) -> String {
    let mut code = HEADER.comment::<Rust>();
    code.push_str("//! The mod's native: its code is Yog Script.\n\n");
    if content {
        code.push_str("yog_api::content!();\n\n");
    }
    code.push_str(
        "/// The mod: what it registers is its content's.\npub struct ScriptMod;\n\nimpl yog_api::Mod for ScriptMod {\n    fn register(_registry: &mut yog_api::Registry) {}\n}\n\nyog_api::export_mod!(ScriptMod);\n",
    );
    code
}

const HEADER: Header = Header {
    tool: "`yog build`",
    source: "yog.toml",
    regenerate: Regenerate::EveryBuild,
};
