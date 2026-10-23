//! The ids of the resources named by their path — `SoundEvents` of the
//! sound events (`resources/sound/**/*.ysound`), `Particles` of the
//! particles (`resources/particle/**/*.yparticle`) — as consts, named by
//! that path (`casting/cast/fail.ysound` → `CASTING_CAST_FAIL`): the files
//! their one source.

use std::fmt::Write as _;
use std::path::Path;

use yog_ids::Format;

use super::RESOURCES_DIR;

/// The generated types (a `.yres` set can't take their names): each a
/// format's files under their folder.
pub(super) const SETS: [(&str, &str, Format); 2] = [
    ("SoundEvents", yog_formats::sound::DIR, Format::Sound),
    ("Particles", yog_formats::particle::DIR, Format::Particle),
];
/// A const's name: its file's path, words joined by this, upper case.
const WORD_SEP: &str = "_";

/// Type `set`'s code: a const per `format` file under `dir`, by path; none without them.
pub(super) fn source(
    root: &Path,
    mod_id: &str,
    set: &str,
    dir: &str,
    format: Format,
) -> Option<String> {
    let resources = root.join(RESOURCES_DIR);
    let mut events: Vec<(String, String)> = crate::files::with_ext(&resources, format.ext())
        .iter()
        .filter_map(|f| {
            let rel = f
                .strip_prefix(&resources)
                .ok()?
                .to_string_lossy()
                .replace('\\', yog_ids::sep::PATH);
            let id = yog_formats::resource_id(mod_id, &rel, dir, format)?;
            Some((rel, id))
        })
        .collect();
    if events.is_empty() {
        return None;
    }
    events.sort();
    let ext = format.ext();
    let mut code = format!(
        "/// The ids of `{RESOURCES_DIR}/{dir}/`'s `.{ext}` files, by path.\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code)]\nimpl {set} {{\n"
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
        let (set, d, f) = super::SETS[0];
        assert!(super::source(&dir.join("nothing"), "m", set, d, f).is_none());
        let (set, d, f) = super::SETS[0];
        let code = super::source(&dir, "m", set, d, f).unwrap();
        assert!(
            code.contains("pub const CAST_FAIL: &'static str = \"m:cast.fail\";"),
            "{code}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
