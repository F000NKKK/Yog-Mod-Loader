//! The game's kinds of block (`kind: "stairs"`): their template models with
//! the `.yblock`'s textures, and their states.

use serde_json::{Map, Value, json};
use yog_data::Path as At;

use crate::content::formats::block_item::Emit;
use crate::content::formats::model::state::{state_properties, variant};
use crate::content::pipeline::ctx::{Ctx, Loc};

/// A block of one of the game's kinds: the kind, what it's made of, its
/// render layer.
pub(in crate::content) struct KindBlock<'a> {
    pub(in crate::content) kind: &'a yog_formats::content::BlockKind,
    pub(in crate::content) base: Option<&'a str>,
    pub(in crate::content) render_layer: Option<&'a str>,
}

/// `layer`'s render type on a block's model (Forge/NeoForge read it; Fabric ignores it).
pub(in crate::content) fn with_render_type(json: &mut Value, layer: Option<&str>) {
    if let (Some(layer), Value::Object(o)) = (layer, json) {
        o.insert(
            yog_ids::model::RENDER_TYPE.into(),
            Value::String(yog_ids::qualify(layer)),
        );
    }
}

/// A kind's models (the game's templates with the `.yblock`'s textures)
/// and states written; the item's model's id, when the kind has one.
pub(in crate::content) fn kind_models(
    cx: &mut Ctx,
    loc: Loc,
    kb: &KindBlock,
    m: &yog_formats::content::Model,
    emit: &mut Emit,
) -> Option<String> {
    let k = kb.kind;
    if m.parent.is_some()
        || m.elements.is_some()
        || m.display.is_some()
        || m.ambient_occlusion.is_some()
        || m.gui_light.is_some()
    {
        let at = At::root().key(yog_ids::content_field::MODEL);
        cx.err_at(
            loc,
            &at,
            format!(
                "kind {} has the game's models — give only textures ({})",
                k.name,
                k.slots.join(", ")
            ),
        );
    }
    let textures = kind_textures(cx, loc, k, m);
    let names = Names {
        p: emit.p.clone(),
        ns: emit.ns.clone(),
        base: kb.base,
    };
    for (name, parent) in &k.models.0 {
        let mut json =
            json!({ yog_ids::model::PARENT: parent, yog_ids::model::TEXTURES: textures });
        with_render_type(&mut json, kb.render_layer);
        let file = yog_ids::model_file(&format!("{}{}", yog_ids::res::BLOCK_DIR, names.path(name)));
        emit.asset(cx, file, &json);
    }
    emit.asset(
        cx,
        yog_ids::blockstate_file(&names.p),
        &blockstate(k, &names),
    );
    k.item.as_deref().map(|name| names.id(name))
}

/// How a kind's models are named: `""` is the block's own model, `base`
/// its base block's, another name `<block>_<name>`.
struct Names<'a> {
    p: String,
    ns: String,
    base: Option<&'a str>,
}

impl Names<'_> {
    fn path(&self, name: &str) -> String {
        if name.is_empty() {
            self.p.clone()
        } else {
            format!("{}_{name}", self.p)
        }
    }

    fn id(&self, name: &str) -> String {
        let block_model =
            |ns: &str, path: &str| yog_ids::join(ns, &format!("{}{path}", yog_ids::res::BLOCK_DIR));
        match (name, self.base) {
            (BASE, Some(b)) => {
                let (bns, bp) = yog_ids::split(b);
                block_model(bns, bp)
            }
            _ => block_model(&self.ns, &self.path(name)),
        }
    }
}

/// The kind's states as the game's: its variants, or its parts drawn when
/// their properties match.
fn blockstate(k: &yog_formats::content::BlockKind, names: &Names) -> Value {
    let shown = |km: &yog_formats::content::KindModel| {
        variant(
            &names.id(&km.model),
            f64::from(km.x.unwrap_or(0)),
            f64::from(km.y.unwrap_or(0)),
            km.uvlock,
            None,
        )
    };
    if k.multipart.is_empty() {
        let variants: Map<String, Value> = k
            .variants
            .0
            .iter()
            .map(|(state, km)| (state.clone(), shown(km)))
            .collect();
        return json!({ yog_ids::model::VARIANTS: variants });
    }
    let parts: Vec<Value> = k
        .multipart
        .iter()
        .map(|part| {
            let mut o = Map::new();
            o.insert(yog_ids::model::APPLY.into(), shown(&part.apply));
            if let Some(when) = &part.when {
                o.insert(
                    yog_ids::model::WHEN.into(),
                    Value::Object(state_properties(when)),
                );
            }
            Value::Object(o)
        })
        .collect();
    json!({ yog_ids::model::MULTIPART: parts })
}

/// A kind's model name for the model of the block it's made of.
const BASE: &str = "base";

/// The kind's texture slots from the `.yblock`'s textures; a slot it hasn't
/// a warning, one it misses an error.
fn kind_textures(
    cx: &mut Ctx,
    loc: Loc,
    k: &yog_formats::content::BlockKind,
    m: &yog_formats::content::Model,
) -> Map<String, Value> {
    let at = At::root().key(yog_ids::content_field::MODEL);
    let mut textures = Map::new();
    for (slot, tex) in &m.textures.0 {
        let here = at.key(yog_ids::model::TEXTURES).key(slot);
        if slot != yog_ids::model::PARTICLE && !k.slots.contains(slot) {
            cx.warn_at(
                loc,
                &here,
                format!(
                    "kind {} has no slot `{slot}` ({})",
                    k.name,
                    k.slots.join(", ")
                ),
            );
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
        }
    }
    for slot in k.slots.iter().filter(|s| !textures.contains_key(*s)) {
        cx.err_at(loc, &at, format!("kind {} needs textures.{slot}", k.name));
    }
    textures
}
