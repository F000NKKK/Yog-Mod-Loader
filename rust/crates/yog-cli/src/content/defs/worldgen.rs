//! `.ydef` trees, ores and dimensions: the game's worldgen data, the
//! loaders' biome modifiers, the dimensions the runtime reads.

mod consts;
use consts as wg;

use serde_json::{Value, json};
use yog_formats::def::{DimensionDef, HeightShape, OreDef, TreeDef, WeightedBlock};
use yog_ids::dimension_field;
use yog_ids::kind::{BLOCK, TAG as TAG_FIELD};
use yog_ids::loader::{FORGE, NEOFORGE};
use yog_ids::sep::{NAMESPACE, PATH, TAG, WORD};

use crate::content::Output;
use crate::content::datapack;
use crate::content::file::generated;
use crate::content::pretty;

/// What an ore makes: its files, and the biome feature (feature, biomes, step) for Fabric's host.
pub(in crate::content) type OreOut = (Vec<Output>, (String, String, String));

/// A configured feature's file and a placed feature's.
fn feature_file(folder: &str, id: &str) -> String {
    datapack::data_in(&[yog_ids::pack::WORLDGEN, folder].join(PATH), id)
}

/// The block state `{ "Name": block }`.
fn state(block: &str) -> Value {
    json!({ "Name": block })
}

/// One block, or blocks by weight.
fn provider(blocks: &[WeightedBlock]) -> Value {
    match blocks {
        [one] => json!({ "type": wg::SIMPLE_STATE_PROVIDER, "state": state(&one.block.id) }),
        many => json!({
            "type": wg::WEIGHTED_STATE_PROVIDER,
            "entries": many.iter().map(|b| json!({ "data": state(&b.block.id), "weight": b.weight })).collect::<Vec<_>>()
        }),
    }
}

/// A tree part's placer: the game's by its short name (`fancy`), a mod's by id.
fn placer(shape: &str, part: &str) -> String {
    if shape.contains(NAMESPACE) {
        shape.to_string()
    } else {
        yog_ids::join(
            yog_ids::ns::MINECRAFT,
            &[shape, part, wg::PLACER].join(WORD),
        )
    }
}

/// How big a tree must have room to be: two layers, or clipped at a height.
fn minimum_size(min_clipped_height: Option<u32>) -> Value {
    match min_clipped_height {
        None => {
            json!({ "type": wg::TWO_LAYERS_FEATURE_SIZE, "limit": 1, "lower_size": 0, "upper_size": 1 })
        }
        Some(h) => {
            json!({ "type": wg::TWO_LAYERS_FEATURE_SIZE, "limit": 0, "lower_size": 0, "upper_size": 0, "min_clipped_height": h })
        }
    }
}

/// What goes under the trunk: the dirt, but not over what can't be replaced (unless forced).
fn below_trunk(dirt: &Value, force: bool) -> Value {
    if force {
        return dirt.clone();
    }
    json!({ "type": wg::RULE_BASED_STATE_PROVIDER, "rules": [{
        "if_true": { "type": wg::NOT, "predicate": { "type": wg::MATCHING_BLOCK_TAG, TAG_FIELD: wg::CANNOT_REPLACE_BELOW_TREE_TRUNK } },
        "then": dirt,
    }] })
}

/// Tree `id` (the mod's own, checked): the game's tree configured feature.
pub(super) fn tree(t: &TreeDef, id: &str) -> Result<Output, String> {
    if t.trunk.blocks.is_empty() || t.leaves.blocks.is_empty() {
        return Err("a tree needs trunk and leaves blocks".into());
    }
    let mut foliage = json!({ "type": placer(&t.leaves.shape, wg::FOLIAGE), "radius": t.leaves.radius, "offset": t.leaves.offset });
    if let Some(h) = t.leaves.height {
        foliage["height"] = h.into();
    }
    let dirt = json!({ "type": wg::SIMPLE_STATE_PROVIDER, "state": state(t.dirt.as_ref().map_or(wg::DIRT, |b| b.id.as_str())) });
    let below = below_trunk(&dirt, t.force_dirt);
    let v = json!({
        "type": wg::TREE,
        "config": {
            "trunk_provider": provider(&t.trunk.blocks),
            "trunk_placer": { "type": placer(&t.trunk.shape, wg::TRUNK), "base_height": t.trunk.base_height, "height_rand_a": t.trunk.random_a, "height_rand_b": t.trunk.random_b },
            "foliage_provider": provider(&t.leaves.blocks),
            "foliage_placer": foliage,
            "dirt_provider": dirt,
            // 26.x: what goes under the trunk.
            "below_trunk_provider": below,
            "minimum_size": minimum_size(t.min_clipped_height),
            "decorators": [],
            "ignore_vines": t.ignore_vines,
            "force_dirt": t.force_dirt
        }
    });
    Ok((feature_file(wg::CONFIGURED_FEATURE, id), pretty(&v)))
}

/// Where dimensions' data is kept under `.yog-build/generated/`.
const DIMENSIONS_DIR: &str = "dimensions";

/// A dimension's data file under `.yog-build/generated/`.
pub(in crate::content) fn dimension_path(id: &str) -> String {
    generated(DIMENSIONS_DIR, id, yog_ids::Format::Data)
}

/// A dimension as the runtime and the hosts read it (Yog Data, binary):
/// its type, biome, flags, the generator that makes its chunks — its
/// `terrain`'s (the runtime's own, under the dimension's id) or the mod's.
pub(super) fn dimension(d: DimensionDef, ns: &str) -> (String, Vec<u8>) {
    use yog_data::{Map, Schema, Value as V};
    let id = yog_ids::qualify_in(ns, &d.id);
    let mut m = Map::new();
    m.insert(dimension_field::ID, V::from(id.as_str()));
    m.insert(dimension_field::DIMENSION_TYPE, d.dimension_type.write());
    m.insert(
        dimension_field::BIOME,
        V::from(
            d.biome
                .map_or(yog_ids::game::PLAINS.to_string(), |b| b.id)
                .as_str(),
        ),
    );
    m.insert(dimension_field::STRUCTURES, V::Bool(d.structures));
    m.insert(dimension_field::FEATURES, V::Bool(d.features));
    m.insert(dimension_field::MOB_SPAWNING, V::Bool(d.mob_spawning));
    if let Some(t) = &d.terrain {
        m.insert(dimension_field::GENERATOR_TYPE, V::from(id.as_str()));
        m.insert(dimension_field::TERRAIN, t.write());
    } else if let Some(g) = &d.generator {
        m.insert(
            dimension_field::GENERATOR_TYPE,
            V::from(yog_ids::qualify_in(ns, g).as_str()),
        );
    }
    (id, yog_data::binary::encode(&V::Map(m)))
}

/// The ore's configured feature: veins of its blocks in place of others.
fn configured_ore(o: &OreDef) -> Value {
    let targets: Vec<Value> = o
        .targets
        .iter()
        .map(|t| {
            let target = match t.replaces.strip_prefix(TAG) {
                Some(tag) => json!({ "predicate_type": wg::TAG_MATCH, TAG_FIELD: tag }),
                None => json!({ "predicate_type": wg::BLOCK_MATCH, BLOCK: t.replaces }),
            };
            json!({ "target": target, "state": state(&t.block.id) })
        })
        .collect();
    json!({
        "type": wg::ORE,
        "config": { "size": o.size, "discard_chance_on_air_exposure": o.air_exposure, "targets": targets }
    })
}

/// The ore's placed feature `id`: so many veins a chunk, between its heights.
fn placed_ore(o: &OreDef, id: &str) -> Value {
    let shape = match o.height.shape {
        HeightShape::Uniform => wg::UNIFORM,
        HeightShape::Triangle => wg::TRAPEZOID,
    };
    json!({
        "feature": id,
        "placement": [
            { "type": wg::COUNT, "count": o.per_chunk },
            { "type": wg::IN_SQUARE },
            { "type": wg::HEIGHT_RANGE, "height": { "type": shape, "min_inclusive": { "absolute": o.height.min }, "max_inclusive": { "absolute": o.height.max } } },
            { "type": wg::BIOME }
        ]
    })
}

/// An ore: its configured and placed features (the same on every version),
/// the biome modifiers Forge and NeoForge read (each its own), and the
/// biome feature the build registers for Fabric's host.
pub(super) fn ore(o: OreDef, ns: &str) -> OreOut {
    let id = yog_ids::join(ns, yog_ids::split(&o.id).1);
    let biomes = o
        .biomes
        .as_ref()
        .map_or_else(|| wg::IS_OVERWORLD.to_string(), |b| b.id.clone());
    let modifier = |kind: &str| json!({ "type": kind, "biomes": format!("{TAG}{biomes}"), "features": id, "step": wg::UNDERGROUND_ORES });
    let modifier_file =
        |loader: &str| datapack::data_in(&[loader, wg::BIOME_MODIFIER_DIR].join(PATH), &id);
    let in_overlay = |loader: &str, version: &str| [loader, version].join(PATH);
    let files: Vec<Output> = vec![
        (
            feature_file(wg::CONFIGURED_FEATURE, &id),
            pretty(&configured_ore(&o)),
        ),
        (
            feature_file(wg::PLACED_FEATURE, &id),
            pretty(&placed_ore(&o, &id)),
        ),
        (
            datapack::overlay(
                &in_overlay(FORGE, yog_ids::pack::ANY_VERSION),
                &modifier_file(FORGE),
            ),
            pretty(&modifier(wg::BIOME_MODIFIER_FORGE_ADD_FEATURES)),
        ),
        (
            datapack::overlay(
                &in_overlay(NEOFORGE, yog_ids::versions::NEOFORGE_READS_FORGE),
                &modifier_file(FORGE),
            ),
            pretty(&modifier(wg::BIOME_MODIFIER_FORGE_ADD_FEATURES)),
        ),
        (
            datapack::overlay(
                &in_overlay(NEOFORGE, yog_ids::versions::NEOFORGE_READS_OWN),
                &modifier_file(NEOFORGE),
            ),
            pretty(&modifier(wg::BIOME_MODIFIER_NEOFORGE_ADD_FEATURES)),
        ),
    ];
    (files, (id, biomes, wg::UNDERGROUND_ORES.into()))
}
