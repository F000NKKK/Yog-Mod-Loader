//! The tab as the editor hands it — its files' values as written — and
//! its texts as a reader of the mod sees them.

use std::path::Path;

use yog_data::Text;
use yog_data::kinds::{Block, Item};
use yog_data::with::{Opt, RefTo};

use super::super::assets::Resources;
use super::{Args, Node};

/// One advancement as the editor hands it: its file's values as written.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct NodeArgs {
    pub id: String,
    pub parent: Option<yog_data::Id<yog_data::kinds::Advancement>>,
    /// An item's or a block's (`@item:…`).
    #[data(with = "Opt<RefTo<(Item, Block)>>")]
    pub icon: Option<String>,
    /// `task`, `goal` or `challenge`.
    pub frame: String,
    pub title: Option<Text>,
    pub description: Option<Text>,
    /// Drawn as had (the one being edited).
    #[data(default)]
    pub obtained: bool,
}

/// What to draw, as the editor hands it.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct AdvancementArgs {
    pub path: String,
    pub title: Option<Text>,
    /// The root's background texture (`minecraft:textures/block/calcite.png` or `minecraft:block/calcite`).
    pub background: Option<String>,
    pub nodes: Vec<NodeArgs>,
    /// How far the reader dragged the tree, GUI pixels.
    #[data(default)]
    pub drag: [f32; 2],
    /// The advancement under the pointer.
    pub hover: Option<String>,
}

impl AdvancementArgs {
    /// Its texts as a reader of the mod sees them (the neutral locale).
    pub fn resolved(&self) -> Args {
        let root = super::super::mod_root(Path::new(&self.path));
        let res = Resources::for_mod(root.as_deref());
        let texts = super::super::book::texts(root.as_deref(), &res, None);
        let shown = |t: &Option<Text>, or: &str| {
            t.as_ref()
                .map_or_else(|| or.to_string(), |t| shown(t, &texts))
        };
        let nodes = self
            .nodes
            .iter()
            .map(|n| Node {
                id: n.id.clone(),
                parent: n.parent.as_ref().map(|p| yog_ids::qualify(&p.id)),
                icon: n.icon.clone(),
                frame: n.frame.clone(),
                title: shown(&n.title, &n.id),
                description: shown(&n.description, ""),
                obtained: n.obtained,
            })
            .collect();
        let root_id = self
            .nodes
            .iter()
            .find(|n| n.parent.is_none())
            .map_or("", |n| n.id.as_str());
        Args {
            path: self.path.clone(),
            title: shown(&self.title, root_id),
            background: self.background.clone(),
            nodes,
            drag: self.drag,
            hover: self.hover.clone(),
        }
    }
}

/// `t` as a reader sees it: a `.yres` text or a key translated, a name looked up.
fn shown(t: &Text, texts: &super::super::book::Texts) -> String {
    match t {
        Text::Literal(s) => s.clone(),
        Text::Res(r) => texts
            .refs
            .get(&format!("{}.{}", r.set, r.key))
            .cloned()
            .unwrap_or_else(|| r.to_string()),
        Text::Lang(key) => texts.lang.get(key).cloned().unwrap_or_else(|| key.clone()),
        Text::NameOf(r) => {
            super::super::book::item_name(&texts.lang, &r.id).unwrap_or_else(|| r.id.clone())
        }
    }
}
