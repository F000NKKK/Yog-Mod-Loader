//! Kinds of recipe for the recipe editor — the game's (`yog_registry`'s
//! built-in definitions) and the mod's own (`.ydef` `kind: "recipe_kind"`),
//! all described the same way: their slots (and the `.yrecipe` fields the
//! game's are written in), their values, and — the layout books show them
//! by (built in, or a `.yui` `recipe_layout`) — the picture (images, the
//! station, slot frames) with where slots, result and texts go on it.

use std::path::{Path, PathBuf};
use yog_ids::Format;

use base64::Engine as _;
use serde::Serialize;
use yog_book::layout_picture::{Piece, pieces};
use yog_book::renderer::{BLOCK_PITCH, SLOT_YAW};
use yog_data::Schema;
use yog_formats::book::layout::{LayoutPart, RecipeLayout};
use yog_formats::def::{Def, RecipeKindDef};
use yog_gfx::{Color, Vec2};
use yog_registry::recipe_kinds::{self, fields};

use crate::design::{assets::Resources, gfx, images, raster};

/// How much bigger than GUI pixels the picture is drawn.
const SCALE: u32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub name: String,
    /// What editors call it; `name` if none.
    pub label: Option<String>,
    pub description: Option<String>,
    pub max: u32,
    pub required: bool,
    /// The game's kinds: the `.yrecipe` field it's written in.
    pub field: Option<String>,
    /// Its places are a crafting grid's (`pattern` and `key`).
    pub grid: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSpec {
    pub name: String,
    /// `number`, `text` or `bool` — the format's word for its type.
    pub kind: String,
    /// What it is when a recipe doesn't say.
    pub default: Option<f64>,
}

/// Something the editor puts on the picture, in GUI pixels.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Spot {
    /// The `index`-th ingredient of `slot`.
    Slot {
        slot: String,
        index: usize,
        at: [f32; 2],
    },
    Result {
        at: [f32; 2],
    },
    Count {
        at: [f32; 2],
    },
    /// `{name}` is the recipe's value of that name.
    Text {
        text: String,
        at: [f32; 2],
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// The `.yui` it's laid out in; empty for a built-in layout.
    pub file: String,
    pub size: [f32; 2],
    /// The picture, `SCALE`× GUI pixels, `data:image/png;base64,…`.
    pub image: Option<String>,
    pub scale: u32,
    pub spots: Vec<Spot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kind {
    pub id: String,
    /// What editors call it; the id's path if none.
    pub label: Option<String>,
    /// The `.ydef` that declares it; empty for the game's.
    pub file: String,
    pub slots: Vec<Slot>,
    pub values: Vec<ValueSpec>,
    pub layout: Option<Layout>,
}

/// The fields every `.yrecipe` has, by the names the format gives them
/// (`yog_registry::recipe_kinds::fields`) — the editor reads and writes
/// recipes with these.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fields {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub result: &'static str,
    pub count: &'static str,
    pub slots: &'static str,
    pub values: &'static str,
    pub pattern: &'static str,
    pub key: &'static str,
    pub grid_side: usize,
}

/// Every kind the editor knows for the mod at `root`, and the fields.
#[derive(Debug, Serialize)]
pub struct Kinds {
    pub game: Vec<Kind>,
    pub own: Vec<Kind>,
    pub fields: Fields,
}

pub const FIELDS: Fields = Fields {
    kind: fields::TYPE,
    result: fields::RESULT,
    count: fields::COUNT,
    slots: fields::SLOTS,
    values: fields::VALUES,
    pattern: fields::PATTERN,
    key: fields::KEY,
    grid_side: recipe_kinds::GRID_SIDE,
};

/// The kinds for the mod at `root` (none of the mod's without one).
pub fn all(root: Option<&Path>) -> Kinds {
    Kinds {
        game: root.map(game_kinds).unwrap_or_default(),
        own: root.map(kinds).unwrap_or_default(),
        fields: FIELDS,
    }
}

/// The game's kinds, as books show them: their built-in layouts, or the
/// mod's own `recipe_layout` for one.
pub fn game_kinds(root: &Path) -> Vec<Kind> {
    let layouts = crate::design::mod_ui::recipe_layouts(root);
    recipe_kinds::GAME
        .iter()
        .map(|k| describe(root, k, k.id.clone(), String::new(), &layouts))
        .collect()
}

/// The kinds the mod at `root` declares, by id.
pub fn kinds(root: &Path) -> Vec<Kind> {
    let ns = crate::design::project::mod_id(root).unwrap_or_default();
    let files = crate::files::of(&root.join(yog_ids::project::CONTENT), Format::Def);
    let layouts = crate::design::mod_ui::recipe_layouts(root);
    let mut out: Vec<Kind> = files
        .iter()
        .filter_map(|file| {
            let text = std::fs::read_to_string(file).ok()?;
            let Def::RecipeKind(k) = yog_data::from_text::<Def>(&text).ok()?.value else {
                return None;
            };
            let id = if k.id.contains(':') {
                k.id.clone()
            } else {
                format!("{ns}:{}", k.id)
            };
            Some(describe(
                root,
                &k,
                id,
                file.to_string_lossy().into_owned(),
                &layouts,
            ))
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Kind `k` (as `id`, declared in `file`) for the editor: the mod's layout
/// for it, else a built-in one.
fn describe(
    root: &Path,
    k: &RecipeKindDef,
    id: String,
    file: String,
    layouts: &[(PathBuf, RecipeLayout)],
) -> Kind {
    let own: Vec<RecipeLayout> = layouts.iter().map(|(_, l)| l.clone()).collect();
    let layout_file = layouts
        .iter()
        .find(|(_, l)| l.kinds.contains(&id))
        .map(|(f, _)| f.clone())
        .unwrap_or_default();
    Kind {
        label: k.name.clone(),
        file,
        slots: k
            .slots
            .0
            .iter()
            .map(|(name, s)| Slot {
                name: name.clone(),
                label: s.name.clone(),
                description: s.description.clone(),
                max: s.max,
                required: s.required,
                field: s.field.clone(),
                grid: s.grid,
            })
            .collect(),
        values: k
            .values
            .0
            .iter()
            .map(|(name, t)| ValueSpec {
                name: name.clone(),
                kind: t.write().as_str().unwrap_or_default().to_string(),
                default: k.default_of(name),
            })
            .collect(),
        layout: yog_formats::book::layout::find(&own, &id).map(|l| layout(root, &layout_file, l)),
        id,
    }
}

/// The picture of `layout` (what doesn't change with the recipe) and the spots.
fn layout(root: &Path, file: &Path, layout: &RecipeLayout) -> Layout {
    let size = layout.size;
    let mut spots = Vec::new();
    for p in &layout.parts {
        match p {
            LayoutPart::Slots { slot, at, .. } => {
                spots.extend(at.iter().enumerate().map(|(index, at)| Spot::Slot {
                    slot: slot.clone(),
                    index,
                    at: *at,
                }));
            }
            LayoutPart::Result { at, .. } => spots.push(Spot::Result { at: *at }),
            LayoutPart::Count { at } => spots.push(Spot::Count { at: *at }),
            LayoutPart::Text { text, at } => spots.push(Spot::Text {
                text: text.clone(),
                at: *at,
            }),
            LayoutPart::Image { .. } | LayoutPart::Item { .. } | LayoutPart::Block { .. } => {}
        }
    }
    let (w, h) = (
        size[0].ceil().max(1.0) as u32,
        size[1].ceil().max(1.0) as u32,
    );
    let ops = gfx::record(w as i32, h as i32, |ctx| {
        let d = ctx.draw2d();
        for piece in pieces(layout) {
            match piece {
                Piece::Image { source, uv, at } => {
                    d.image_region(source, at, Some(uv), Color::WHITE)
                }
                Piece::Item { item, at } => d.item(item, Vec2::new(at.x, at.y), at.w),
                Piece::Block { block, at } => {
                    d.blocks(&[((0, 0, 0), block)], at, SLOT_YAW, BLOCK_PITCH)
                }
            }
        }
    });
    images::set_document(Some(root));
    let res = Resources::for_mod(Some(root));
    let image = raster::rasterize(&ops, w, h, SCALE, &res)
        .png()
        .ok()
        .map(|png| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            )
        });
    Layout {
        file: file.to_string_lossy().into_owned(),
        size,
        image,
        scale: SCALE,
        spots,
    }
}

/// Where a new kind's layout goes, under the mod's sources, and what its name ends with.
const LAYOUT_DIR: &str = "ui";
const LAYOUT_SUFFIX: &str = "_layout";

/// Makes a new kind of recipe in the mod of `file` — `content/<name>.ydef`
/// (its slots and values) and `src/ui/<name>_layout.yui` (how it's shown);
/// the layout's path, to open in the designer.
pub fn create(file: &Path, name: &str) -> Result<PathBuf, String> {
    let root = super::mod_root(file).ok_or("not inside a mod")?;
    let name = name.trim();
    if !yog_ids::valid_name(name) {
        return Err("a kind's name: a-z, 0-9 and _".into());
    }
    let def = root
        .join(yog_ids::project::CONTENT)
        .join(Format::Def.file(name));
    let layout = root
        .join(yog_ids::project::SRC)
        .join(LAYOUT_DIR)
        .join(Format::Ui.file(&format!("{name}{LAYOUT_SUFFIX}")));
    if let Some(taken) = [&def, &layout].into_iter().find(|f| f.exists()) {
        return Err(format!("{} exists", taken.display()));
    }
    for (f, text) in [(&def, template(name)), (&layout, layout_template(name))] {
        if let Some(dir) = f.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(f, text).map_err(|e| e.to_string())?;
    }
    Ok(layout)
}

/// A new kind's `.ydef`: two slots and a value to start from.
pub fn template(id: &str) -> String {
    format!(
        r#"{{
  kind: "recipe_kind",
  id: "{id}",
  // Named slots: how many ingredients each takes.
  slots: {{
    input: {{ required: true }},
    extra: {{ max: 2 }},
  }},
  // Named values a recipe of the kind gives (number, text, bool).
  values: {{ time: "number" }},
}}
"#
    )
}

/// A new kind's layout (`src/ui/<id>_layout.yui`), for the screen designer:
/// the template's slots framed, the result, its time.
pub fn layout_template(id: &str) -> String {
    // What the designer shows in them: the game's ore and its ingot.
    let (input, result) = (yog_ids::qualify("iron_ore"), yog_ids::qualify("iron_ingot"));
    format!(
        r#"<!-- How recipes of {id} are shown: in books and in the recipe editor. -->
<Component recipe_layout="{id}" width="100" height="40">
    <RecipeSlot slot="input" x="4" y="12" frame="true" d:item="{input}"/>
    <RecipeSlot slot="extra" x="30" y="4" frame="true"/>
    <RecipeSlot slot="extra" x="30" y="22" frame="true"/>
    <RecipeResult x="78" y="12" frame="true" d:item="{result}"/>
    <Label text="{{x:Bind time}} ticks" x="50" y="32"/>
</Component>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_mods_kinds_and_lays_them_out() {
        let dir = std::env::temp_dir().join(format!("yog-idle-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/machines")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(
            dir.join("content/machines/crusher.ydef"),
            template("crusher"),
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src/ui")).unwrap();
        std::fs::write(
            dir.join("src/ui/crusher_layout.yui"),
            layout_template("crusher"),
        )
        .unwrap();
        std::fs::write(
            dir.join("content/other.ydef"),
            r#"{ kind: "sound_group", id: "x" }"#,
        )
        .unwrap();
        let ks = kinds(&dir);
        assert_eq!(ks.len(), 1);
        let k = &ks[0];
        assert_eq!(k.id, "mymod:crusher");
        assert_eq!(
            k.slots
                .iter()
                .map(|s| (s.name.as_str(), s.max, s.required))
                .collect::<Vec<_>>(),
            [("input", 1, true), ("extra", 2, false)]
        );
        assert_eq!(k.values[0].kind, "number");
        let l = k.layout.as_ref().unwrap();
        assert!(
            l.image
                .as_deref()
                .is_some_and(|i| i.starts_with("data:image/png"))
        );
        assert_eq!(
            l.spots
                .iter()
                .filter(|s| matches!(s, Spot::Slot { .. }))
                .count(),
            3
        );
        assert!(
            l.spots
                .iter()
                .any(|s| matches!(s, Spot::Text { text, .. } if text == "{time} ticks"))
        );
        assert!(l.file.ends_with("crusher_layout.yui"));
        let game = game_kinds(&dir);
        assert_eq!(
            game.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
            recipe_kinds::ids::ALL,
            "the registry's kinds, in its order"
        );
        let shaped = game[0].layout.as_ref().unwrap();
        assert_eq!(
            shaped
                .spots
                .iter()
                .filter(|s| matches!(s, Spot::Slot { slot, .. } if slot == "grid"))
                .count(),
            9,
            "the crafting table's own picture"
        );
        let smithing = game
            .iter()
            .find(|k| k.id == recipe_kinds::ids::SMITHING)
            .unwrap()
            .layout
            .as_ref()
            .unwrap();
        let parts: Vec<&str> = smithing
            .spots
            .iter()
            .filter_map(|s| {
                if let Spot::Slot { slot, .. } = s {
                    Some(slot.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            parts,
            ["base", "addition", "template"],
            "Patchouli's smithing strip"
        );
        assert!(
            smithing
                .spots
                .iter()
                .any(|s| matches!(s, Spot::Result { .. }))
        );
        assert!(smithing.image.is_some());
        let brewing = game
            .iter()
            .find(|k| k.id == recipe_kinds::ids::BREWING)
            .unwrap();
        assert_eq!(
            brewing
                .slots
                .iter()
                .map(|s| s.field.as_deref())
                .collect::<Vec<_>>(),
            [Some("from"), Some("ingredient")]
        );
        assert!(
            brewing.layout.as_ref().is_some_and(|l| l.image.is_some()),
            "Yog's own brewing strip"
        );
        // What the recipe editor reads (recipeModel.ts `RecipeKind`).
        let json = serde_json::to_value(&l.spots).unwrap();
        let types: Vec<&str> = json
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["type"].as_str().unwrap())
            .collect();
        assert_eq!(types, ["slot", "slot", "slot", "result", "text"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
