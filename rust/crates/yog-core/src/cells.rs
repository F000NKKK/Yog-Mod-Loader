//! Positions as mirror cells (`yog_mirror::Cell`): a row of a column is
//! `[x, y, z]`; a block's key in `yog:keys/block` is its packed position.

use yog_mirror::{Cell, Kind};

use crate::{BlockPos, Point};

/// The game's packed block position (`BlockPos.asLong`): x and z 26 bits,
/// y 12, as `x | z | y` from the top.
const XZ_BITS: u32 = 26;
const Y_BITS: u32 = 12;
const X_SHIFT: u32 = Y_BITS + XZ_BITS;
const Z_SHIFT: u32 = Y_BITS;

const fn mask(bits: u32) -> i64 {
    (1 << bits) - 1
}

/// The signed `bits`-wide field at `shift` of `p`: shifted to the top,
/// then back, so the sign comes along.
const fn field(p: i64, shift: u32, bits: u32) -> i32 {
    ((p << (i64::BITS - shift - bits)) >> (i64::BITS - bits)) as i32
}

impl BlockPos {
    /// Its packed form: the game's `BlockPos.asLong`, its key in `yog:keys/block`.
    pub const fn packed(self) -> u64 {
        (((self.x as i64 & mask(XZ_BITS)) << X_SHIFT)
            | ((self.z as i64 & mask(XZ_BITS)) << Z_SHIFT)
            | (self.y as i64 & mask(Y_BITS))) as u64
    }

    /// The position `packed` holds.
    pub const fn unpacked(packed: u64) -> BlockPos {
        let p = packed as i64;
        BlockPos::new(
            field(p, X_SHIFT, XZ_BITS),
            field(p, 0, Y_BITS),
            field(p, Z_SHIFT, XZ_BITS),
        )
    }
}

impl Cell for Point {
    const KIND: Kind = Kind::F64;
    const COUNT: u32 = 3;

    #[inline]
    fn put(&self, out: &mut [u8]) {
        [self.x, self.y, self.z].put(out);
    }

    #[inline]
    fn take(bytes: &[u8]) -> Self {
        let [x, y, z] = <[f64; 3]>::take(bytes);
        Point { x, y, z }
    }
}

impl Cell for BlockPos {
    const KIND: Kind = Kind::I32;
    const COUNT: u32 = 3;

    #[inline]
    fn put(&self, out: &mut [u8]) {
        <[i32; 3]>::from(*self).put(out);
    }

    #[inline]
    fn take(bytes: &[u8]) -> Self {
        <[i32; 3]>::take(bytes).into()
    }
}

#[cfg(test)]
mod tests;
