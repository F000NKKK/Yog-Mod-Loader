//! Block states: how a state turns its model, one variant's JSON, and a
//! state's properties.

use serde_json::{Map, Value};
use yog_ids::block_state::{CEILING, WALL};
use yog_ids::direction::{DOWN, EAST, SOUTH, UP, WEST};
use yog_ids::model as m;
use yog_ids::sep::{PROPERTY_IS, PROPERTY_SEP};

use crate::content::num_value;

/// A quarter turn, a half and a whole, in degrees.
const QUARTER: f64 = 90.0;
const HALF: f64 = 180.0;
const WHOLE: f64 = 360.0;

/// How a state turns its model (x, then y, in degrees): `facing` is where
/// its front (the model's north side) looks; `face`, where an attached
/// block (its model lying on the floor) hangs.
pub(in crate::content) fn turn(facing: Option<&str>, face: Option<&str>) -> (f64, f64) {
    let yaw = |f: &str| match f {
        EAST => QUARTER,
        SOUTH => HALF,
        WEST => HALF + QUARTER,
        _ => 0.0,
    };
    match (face, facing) {
        (Some(WALL), Some(f)) => (QUARTER, yaw(f)),
        (Some(CEILING), Some(f)) => (HALF, (yaw(f) + HALF) % WHOLE),
        (_, Some(UP)) => (HALF + QUARTER, 0.0),
        (_, Some(DOWN)) => (QUARTER, 0.0),
        (_, Some(f)) => (0.0, yaw(f)),
        _ => (0.0, 0.0),
    }
}

/// The JSON of one block state: `{"model": ..., "x": .., "y": .., ...}`.
pub(in crate::content) fn variant(
    model: &str,
    x: f64,
    y: f64,
    uvlock: bool,
    weight: Option<f64>,
) -> Value {
    let mut v = Map::new();
    v.insert(m::MODEL.into(), Value::String(model.into()));
    if x != 0.0 {
        v.insert(m::X.into(), num_value(x));
    }
    if y != 0.0 {
        v.insert(m::Y.into(), num_value(y));
    }
    if uvlock {
        v.insert(m::UVLOCK.into(), Value::Bool(true));
    }
    if let Some(w) = weight {
        v.insert(m::WEIGHT.into(), num_value(w));
    }
    Value::Object(v)
}

/// `half=lower,open=true` → `{"half": "lower", "open": "true"}`.
pub(in crate::content) fn state_properties(state: &str) -> Map<String, Value> {
    state
        .split(PROPERTY_SEP)
        .filter_map(|pair| pair.split_once(PROPERTY_IS))
        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
        .collect()
}
