//! A game font made the mod's own to edit: Yog's alias of it
//! (`yog:font/alt`) → its copy, `content/font/alt.yfont`, made by
//! `yog import font` unless it's there already.

use std::path::{Path, PathBuf};

use serde::Serialize;
use yog_ids::Format;

use super::project;

/// The mod's copy of a game font: its id, and its file.
#[derive(Serialize)]
pub struct Copy {
    pub id: String,
    pub file: PathBuf,
}

/// The mod (of `file`)'s copy of the game font `alias` names, made first
/// if it has none.
pub fn copy(file: &Path, alias: &str) -> Result<Copy, String> {
    let root = project::mod_root(file).ok_or("not in a mod (no yog.toml)")?;
    let mod_id = project::mod_id(&root).ok_or("the mod has no id")?;
    let game = yog_ids::game_font_of(alias)
        .ok_or_else(|| format!("{alias} isn't Yog's alias of a game font"))?;
    let path = crate::content::font_copy_path(yog_ids::split(&game).1);
    let copy = Copy {
        id: yog_ids::join(&mod_id, &path),
        file: root
            .join(yog_ids::project::CONTENT)
            .join(Format::Font.file(&path)),
    };
    if copy.file.is_file() {
        return Ok(copy);
    }
    let out = super::this_cli()
        .current_dir(&root)
        .args(["import", yog_ids::kind::FONT, alias])
        .output()
        .map_err(|e| format!("failed to run yog: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr)
            .replace("yog: error: ", "")
            .trim()
            .to_string());
    }
    Ok(copy)
}
