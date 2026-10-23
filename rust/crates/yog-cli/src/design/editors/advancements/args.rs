//! The tab as the editor hands it — its files' values as written — and
//! its texts as a reader of the mod sees them.

use std::path::Path;

use yog_data::Text;
use yog_data::kinds::{Block, Item};
use yog_data::with::{Opt, RefTo};

use crate::design::drawing::assets::Resources;
use crate::design::editors::advancements::{Args, Node};

/// One advancement as the editor hands it: its file's values as written.
#[derive(Debug, Clone, yog_data::Schema)]
pub(crate) struct NodeArgs {
    /// Its id, namespaced.
    pub(crate) id: String,
    pub(crate) parent: Option<yog_data::Id<yog_data::kinds::Advancement>>,
    /// An item's or a block's (`@item:…`).
    #[data(with = "Opt<RefTo<(Item, Block)>>")]
    pub(crate) icon: Option<String>,
    /// `task`, `goal` or `challenge` (none, or another: `task`).
    pub(crate) frame: Option<String>,
    pub(crate) title: Option<Text>,
    pub(crate) description: Option<Text>,
    /// A root's background texture (`minecraft:textures/block/calcite.png`).
    pub(crate) background: Option<String>,
}

/// What to draw, as the editor hands it: every advancement of the mod
/// (the one being edited as it's edited) — the tab drawn is `selected`'s.
#[derive(Debug, Clone, yog_data::Schema)]
pub(crate) struct AdvancementArgs {
    pub(crate) path: String,
    /// The one being edited: its tab is drawn, it drawn as had.
    pub(crate) selected: String,
    pub(crate) nodes: Vec<NodeArgs>,
    /// How far the reader dragged the tree, GUI pixels.
    #[data(default)]
    pub(crate) drag: [f32; 2],
    /// The advancement under the pointer.
    pub(crate) hover: Option<String>,
}

/// The frames an advancement may have; the first is a missing or unknown one's.
const FRAMES: [&str; 3] = ["task", "goal", "challenge"];

impl AdvancementArgs {
    /// `selected`'s tab, its texts as a reader of the mod sees them (the neutral locale).
    pub(crate) fn resolved(&self) -> Args {
        let root = crate::design::mod_root(Path::new(&self.path));
        let res = Resources::for_mod(root.as_deref());
        let texts = crate::design::editors::book::texts(root.as_deref(), &res, None);
        let shown = |t: &Option<Text>, or: &str| {
            t.as_ref()
                .map_or_else(|| or.to_string(), |t| shown(t, &texts))
        };
        let all = self.with_outer_parents();
        let parent_of = |id: &str| {
            all.iter()
                .find(|n| n.id == id)
                .and_then(|n| n.parent.as_ref())
                .map(|p| yog_ids::qualify(&p.id))
        };
        let tab_root = root_of(&self.selected, &parent_of);
        let tab: Vec<&NodeArgs> = all
            .iter()
            .filter(|n| root_of(&n.id, &parent_of) == tab_root)
            .collect();
        let nodes = tab
            .iter()
            .map(|n| Node {
                id: n.id.clone(),
                parent: n.parent.as_ref().map(|p| yog_ids::qualify(&p.id)),
                icon: n.icon.clone(),
                frame: n
                    .frame
                    .clone()
                    .filter(|f| FRAMES.contains(&f.as_str()))
                    .unwrap_or_else(|| FRAMES[0].to_string()),
                title: shown(&n.title, &n.id),
                description: shown(&n.description, ""),
                obtained: n.id == self.selected,
            })
            .collect();
        let root_node = tab.iter().find(|n| n.id == tab_root);
        Args {
            path: self.path.clone(),
            title: shown(&root_node.and_then(|n| n.title.clone()), &tab_root),
            background: root_node.and_then(|n| n.background.clone()),
            nodes,
            drag: self.drag,
            hover: self.hover.clone(),
        }
    }

    /// The nodes, and a bare one for each parent outside the mod (drawn too).
    fn with_outer_parents(&self) -> Vec<NodeArgs> {
        let mut all = self.nodes.clone();
        for n in &self.nodes {
            let Some(p) = &n.parent else { continue };
            let id = yog_ids::qualify(&p.id);
            if !all.iter().any(|o| o.id == id) {
                all.push(NodeArgs {
                    id,
                    parent: None,
                    icon: None,
                    frame: None,
                    title: None,
                    description: None,
                    background: None,
                });
            }
        }
        all
    }
}

/// The root of `id`'s tab: up its parents while they're there.
fn root_of(id: &str, parent_of: &dyn Fn(&str) -> Option<String>) -> String {
    let mut at = id.to_string();
    // A loop of parents ends somewhere.
    for _ in 0..MAX_DEPTH {
        match parent_of(&at) {
            Some(p) if p != at => at = p,
            _ => break,
        }
    }
    at
}

/// The deepest a tab's tree is followed.
const MAX_DEPTH: usize = 256;

/// `t` as a reader sees it: a `.yres` text or a key translated, a name looked up.
fn shown(t: &Text, texts: &crate::design::editors::book::Texts) -> String {
    match t {
        Text::Literal(s) => s.clone(),
        Text::Res(r) => texts
            .refs
            .get(&format!("{}.{}", r.set, r.key))
            .cloned()
            .unwrap_or_else(|| r.to_string()),
        Text::Lang(key) => texts.lang.get(key).cloned().unwrap_or_else(|| key.clone()),
        Text::NameOf(r) => crate::design::editors::book::item_name(&texts.lang, &r.id)
            .unwrap_or_else(|| r.id.clone()),
    }
}
