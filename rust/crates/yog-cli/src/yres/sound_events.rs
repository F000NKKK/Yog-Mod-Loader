//! `SoundEvents`: the mod's sound events (`resources/sound/**/*.ysound`)
//! as consts of their ids, named by their path (`casting/cast/fail.ysound`
//! → `CASTING_CAST_FAIL`) — the files their one source.

use std::fmt::Write as _;
use std::path::Path;

use yog_ids::Format;

use super::RESOURCES_DIR;

/// The generated type's name (a `.yres` set can't take it).
pub(super) const SET: &str = "SoundEvents";
/// A const's name: its event's path, words joined by this, upper case.
const WORD_SEP: &str = "_";

/// Its code: a const per event, by path; none without events.
pub(super) fn source(root: &Path, mod_id: &str) -> Option<String> {
    let resources = root.join(RESOURCES_DIR);
    let mut events: Vec<(String, String)> = crate::files::with_ext(&resources, Format::Sound.ext())
        .iter()
        .filter_map(|f| {
            let rel = f
                .strip_prefix(&resources)
                .ok()?
                .to_string_lossy()
                .replace('\\', yog_ids::sep::PATH);
            let id = yog_formats::sound::id_of(mod_id, &rel)?;
            Some((rel, id))
        })
        .collect();
    if events.is_empty() {
        return None;
    }
    events.sort();
    let dir = yog_formats::sound::DIR;
    let mut code = format!(
        "/// The sound events of `{RESOURCES_DIR}/{dir}/` (`.ysound`), by id.\n#[allow(dead_code)]\npub struct {SET};\n\n#[allow(dead_code)]\nimpl {SET} {{\n"
    );
    for (rel, id) in events {
        let (_, path) = yog_ids::split(&id);
        let name = path
            .replace(yog_ids::sep::DOT, WORD_SEP)
            .to_ascii_uppercase();
        let _ = writeln!(
            code,
            "    /// `{RESOURCES_DIR}/{rel}`\n    pub const {name}: &'static str = \"{id}\";"
        );
    }
    code.push_str("}\n");
    Some(code)
}

#[cfg(test)]
mod tests {
    #[test]
    fn each_event_is_a_const_named_by_its_path() {
        let dir = std::env::temp_dir().join(format!("yog-sound-events-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sound = dir.join(super::RESOURCES_DIR).join(yog_formats::sound::DIR);
        std::fs::create_dir_all(sound.join("cast")).unwrap();
        std::fs::write(sound.join("cast/fail.ysound"), "{ sounds: [] }").unwrap();
        assert!(super::source(&dir.join("nothing"), "m").is_none());
        let code = super::source(&dir, "m").unwrap();
        assert!(
            code.contains("pub const CAST_FAIL: &'static str = \"m:cast.fail\";"),
            "{code}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
