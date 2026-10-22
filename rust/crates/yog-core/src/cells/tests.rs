//! Positions packed as the game packs them, and kept as cells.

use yog_mirror::Cell;

use crate::{BlockPos, Point};

#[test]
fn a_block_position_packs_like_the_game() {
    // `BlockPos.asLong` of (1, 2, 3) and of (-1, -64, -1).
    assert_eq!(BlockPos::new(1, 2, 3).packed(), (1 << 38) | (3 << 12) | 2);
    for p in [
        BlockPos::new(-1, -64, -1),
        BlockPos::new(30_000_000, 2047, -30_000_000),
        BlockPos::default(),
    ] {
        assert_eq!(BlockPos::unpacked(p.packed()), p);
    }
}

#[test]
fn positions_round_trip_as_cells() {
    let mut b = [0u8; 24];
    let p = Point {
        x: 1.5,
        y: -2.0,
        z: 64.25,
    };
    p.put(&mut b);
    assert_eq!(Point::take(&b), p);
    let at = BlockPos::new(-5, 70, 12);
    at.put(&mut b);
    assert_eq!(BlockPos::take(&b), at);
}
