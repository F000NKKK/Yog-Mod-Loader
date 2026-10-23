//! `yog index <mc>`: the class index of a Minecraft version — what Yog
//! Script is checked against — built from the game's jars into Yog's
//! cache (`yog_cli::class_index`), and how many classes each side has.

use yog_script::index::Side;

/// The words that name it.
pub(super) const WORDS: &[&str] = &["index"];
const USAGE: &str = "usage: yog index <minecraft version>";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let [mc] = rest else {
        return Err(USAGE.into());
    };
    let index = yog_cli::class_index::load(mc)?;
    let count = |side: Side| index.classes.iter().filter(|c| c.side == side).count();
    let methods: usize = index.classes.iter().map(|c| c.methods.len()).sum();
    eprintln!(
        "==> {mc}: {} classes ({} shared, {} client, {} server), {methods} methods",
        index.classes.len(),
        count(Side::Shared),
        count(Side::Client),
        count(Side::Server),
    );
    Ok(())
}
