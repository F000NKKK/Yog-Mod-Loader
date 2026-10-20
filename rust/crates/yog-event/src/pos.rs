//! Positions as messages carry them: a block's `{ x, y, z }`, a point's.

use yog_core::BlockPos;
use yog_data::schema::{Reader, Shape};
use yog_data::with::With;
use yog_data::{Schema, Value, ValueRef};

/// A block position in a payload.
#[derive(Schema)]
#[data(rename = "BlockPos")]
struct PosData {
    x: i32,
    y: i32,
    z: i32,
}

/// [`BlockPos`] through [`PosData`] (yog-core carries no schemas).
pub(crate) struct Pos;

impl With<BlockPos> for Pos {
    fn shape() -> Shape {
        PosData::shape()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<BlockPos> {
        PosData::read(v, cx).map(|p| BlockPos { x: p.x, y: p.y, z: p.z })
    }
    fn write(p: &BlockPos) -> Value {
        PosData { x: p.x, y: p.y, z: p.z }.write()
    }
}

/// A point in a world (blocks, fractional).
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
