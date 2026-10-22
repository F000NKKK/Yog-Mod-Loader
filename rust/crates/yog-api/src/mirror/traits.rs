//! A block's traits as a block mirror writes them (`ClientBlocks::traits`):
//! a bit each, the bit its place in `yog_ids::block_trait`.

use yog_ids::block_trait;

/// A set of block traits.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlockTraits(pub u64);

/// `name`'s bit: its place in `block_trait`'s list.
const fn bit(name: &str) -> u64 {
    let all = block_trait::ALL_VALUES;
    let mut i = 0;
    while i < all.len() {
        if eq(all[i], name) {
            return 1 << i;
        }
        i += 1;
    }
    panic!("not a block trait")
}

const fn eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

macro_rules! traits {
    ($($name:ident),* $(,)?) => {
        impl BlockTraits {
            $(
                #[doc = concat!("`block_trait::", stringify!($name), "`.")]
                pub const $name: BlockTraits = BlockTraits(bit(block_trait::$name));
            )*
            /// How many there are (each of `block_trait`'s).
            #[cfg(test)]
            pub(crate) const COUNT: usize = [$(stringify!($name)),*].len();
        }
    };
}

traits!(
    AIR,
    SOLID,
    FULL,
    LIQUID,
    WATER,
    LAVA,
    SOURCE,
    FLUID_BLOCK,
    CLIMBABLE,
    HARMFUL,
    FALLS,
    PATHFINDABLE,
    REPLACEABLE,
    STURDY_TOP,
    FIRE,
    SLAB,
    STAIRS,
    DOOR,
    WOODEN_DOOR,
    FENCE_GATE,
    TRAPDOOR,
    FENCE,
    WALL,
    CARPET,
    SNOW_LAYER,
    SKULL,
    SHULKER_BOX,
    LEAVES,
    INFESTED,
    CAULDRON,
    AZALEA,
    AMETHYST_CLUSTER,
    POINTED_DRIPSTONE,
    BAMBOO,
    MOVING_PISTON,
    SCAFFOLDING,
    CHEST,
    GLASS,
);

impl BlockTraits {
    /// Whether every one of `t` is among these.
    #[inline]
    pub const fn has(self, t: BlockTraits) -> bool {
        self.0 & t.0 == t.0
    }

    /// Whether any of `t` is among these.
    #[inline]
    pub const fn any(self, t: BlockTraits) -> bool {
        self.0 & t.0 != 0
    }
}

impl std::ops::BitOr for BlockTraits {
    type Output = BlockTraits;

    fn bitor(self, o: BlockTraits) -> BlockTraits {
        BlockTraits(self.0 | o.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trait_has_its_bit() {
        assert_eq!(BlockTraits::COUNT, block_trait::ALL_VALUES.len());
        assert_eq!(BlockTraits::AIR.0, 1);
        assert_eq!(BlockTraits::GLASS.0, 1 << (block_trait::ALL_VALUES.len() - 1));
        assert!((BlockTraits::SLAB | BlockTraits::WATER).has(BlockTraits::SLAB));
    }
}
