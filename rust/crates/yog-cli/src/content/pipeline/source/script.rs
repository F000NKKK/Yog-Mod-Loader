//! The Yog Script the build generates for a mod's content
//! (`yog-content.g.ys`): each set of ids (`Blocks`, `Items`, …) a class of
//! constants by name — what the mod's `.ys` names its content by, the same
//! ids its generated Rust names (`Blocks.Lamp` and `Blocks::LAMP`).

use std::collections::HashMap;

use yog_codegen::{Blocks, Comments, Header, Regenerate};
use yog_codegen_ys::Ys;

use super::{Content, IdSet, all_id_sets, rust_ident, unique_ident};

/// The generated file's header.
const HEADER: Header = Header {
    tool: "`yog build`",
    source: "content/**",
    regenerate: Regenerate::EveryBuild,
};

/// The content's Yog Script; `None` when it has no ids to name.
pub(in crate::content) fn ys_source(c: &Content) -> Option<String> {
    let sets: Vec<IdSet> = all_id_sets(c)
        .into_iter()
        .filter(|s| !s.ids.is_empty())
        .collect();
    if sets.is_empty() {
        return None;
    }
    let mut code = HEADER.comment::<Ys>();
    for set in sets {
        code.push('\n');
        code.push_str(&set_class(&set));
    }
    Some(code)
}

/// One set as a class: a constant per id, named by its path in PascalCase
/// (`ores/ruby`: `OresRuby`), told apart when two read the same.
fn set_class(set: &IdSet) -> String {
    let mut code = format!(
        "{}\n{}\n",
        Ys::doc(&set.doc),
        Ys::open(&Ys::static_class(set.name))
    );
    let mut used: HashMap<String, usize> = HashMap::new();
    for id in &set.ids {
        let name = unique_ident(
            &mut used,
            yog_codegen::pascal(&rust_ident(yog_ids::split(id).1)),
        );
        code.push_str(Ys::INDENT);
        code.push_str(&Ys::const_string(&name, id));
        code.push('\n');
    }
    code.push_str(&Ys::close(""));
    code.push('\n');
    code
}
