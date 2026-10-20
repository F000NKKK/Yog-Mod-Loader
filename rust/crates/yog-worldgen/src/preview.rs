//! Pictures of a terrain for the IDE: from above (the top block's color,
//! shaded by slope) and a slice down through it — from the generator the
//! game runs.

use crate::Generator;

/// The common blocks' colours (by their paths, any namespace), as they look.
const KNOWN: &[(&[&str], u32)] = &[
    (&["grass_block", "moss_block"], 0x5d8a3a),
    (&["dirt", "coarse_dirt", "rooted_dirt"], 0x866043),
    (&["podzol"], 0x5a3e1d),
    (&["stone", "cobblestone", "andesite"], 0x7d7d7d),
    (&["deepslate"], 0x4d4d50),
    (&["granite"], 0x9a6a55),
    (&["diorite"], 0xbcbcbc),
    (&["sand"], 0xdbd3a0),
    (&["red_sand"], 0xbe6621),
    (&["sandstone"], 0xd8cb9b),
    (&["gravel"], 0x837e7d),
    (&["clay"], 0x9ea4b0),
    (&["snow_block", "snow", "powder_snow"], 0xf3f6f6),
    (&["ice", "packed_ice", "blue_ice"], 0x93b4ec),
    (&["water"], 0x3f5fd5),
    (&["lava"], 0xd65a12),
    (&["netherrack"], 0x6f3534),
    (&["soul_sand", "soul_soil"], 0x4f3b2f),
    (&["basalt", "blackstone"], 0x3c3a3e),
    (&["end_stone"], 0xdcdf9e),
    (&["obsidian"], 0x150f22),
    (&["bedrock"], 0x333333),
    (&["mycelium"], 0x6f6265),
    (&["terracotta"], 0x985e43),
    (&["calcite"], 0xdfe0dc),
    (&["tuff"], 0x6c6d66),
    (&["mud"], 0x3c3a3d),
];
/// The colours of whole families, by their paths' ends.
const FAMILIES: &[(&[&str], u32)] = &[
    (&["_ore"], 0x8a8a8a),
    (&["_log", "_wood"], 0x6b5130),
    (&["_leaves"], 0x3e7a2b),
];
/// FNV-1a's offset and prime: an unknown block's steady colour.
const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;
const RGB: u32 = 0x00ff_ffff;
const OPAQUE: u32 = 0xff00_0000;
/// A channel's bits, and where red and green sit.
const CHANNEL: u32 = 0xff;
const RED: u32 = 16;
const GREEN: u32 = 8;

/// An `0xAARRGGBB` color for a block: the common ones as they look, any
/// other a steady color of its own.
pub fn color(block: &str) -> u32 {
    let path = block
        .split_once(yog_ids::sep::NAMESPACE)
        .map_or(block, |(_, p)| p);
    let known = KNOWN
        .iter()
        .find(|(paths, _)| paths.contains(&path))
        .map(|(_, c)| *c);
    let family = || {
        FAMILIES
            .iter()
            .find(|(ends, _)| ends.iter().any(|e| path.ends_with(e)))
            .map(|(_, c)| *c)
    };
    OPAQUE
        | known.or_else(family).unwrap_or_else(|| {
            block
                .bytes()
                .fold(FNV_OFFSET, |h, b| (h ^ b as u32).wrapping_mul(FNV_PRIME))
                & RGB
        })
}

fn shade(c: u32, f: f64) -> u32 {
    let ch = |s: u32| (((c >> s) & CHANNEL) as f64 * f).clamp(0.0, CHANNEL as f64) as u32;
    OPAQUE | (ch(RED) << RED) | (ch(GREEN) << GREEN) | ch(0)
}

/// Light from the north-west: how much brighter a block's rise toward it
/// makes it, and the brightness's bounds.
const SLOPE_LIGHT: f64 = 0.08;
const DARKEST: f64 = 0.6;
const BRIGHTEST: f64 = 1.3;

/// A part of the ground: `w`×`h` blocks from `x`, `z`.
#[derive(Debug, Clone, Copy)]
pub struct Area {
    pub x: i32,
    pub z: i32,
    pub w: u32,
    pub h: u32,
}

/// The terrain from above: a pixel a block of `area`; rows top to bottom
/// (north to south). `min_y`: the world's bottom.
pub fn top_down(g: &Generator, area: Area, min_y: i32) -> Vec<u32> {
    let mut out = Vec::with_capacity((area.w * area.h) as usize);
    for dz in 0..area.h as i32 {
        for dx in 0..area.w as i32 {
            let (x, z) = (area.x + dx, area.z + dz);
            let s = g.surface(x, z);
            // The highest block that isn't air: a fluid over low ground, else the ground's top.
            let (block, fluid) = match &g.terrain.fluid {
                Some(f) if f.level > s => (f.block.as_str(), true),
                _ => (
                    g.block_at(x, s, z, s, min_y).unwrap_or(&g.terrain.fill),
                    false,
                ),
            };
            let slope = if fluid {
                0.0
            } else {
                (s - g.surface(x - 1, z - 1)) as f64
            };
            out.push(shade(
                color(block),
                (1.0 + slope * SLOPE_LIGHT).clamp(DARKEST, BRIGHTEST),
            ));
        }
    }
    out
}

/// A slice down through the terrain along x at `area.z`: `area.w` blocks
/// from `area.x`, from `top` down to `area.h` blocks lower; rows top to
/// bottom. Air is transparent.
pub fn slice(g: &Generator, area: Area, top: i32, min_y: i32) -> Vec<u32> {
    let (w, rows) = (area.w as usize, area.h as usize);
    let mut out = vec![0u32; rows * w];
    for dx in 0..area.w as i32 {
        let x = area.x + dx;
        let s = g.surface(x, area.z);
        for (row, y) in (top + 1 - area.h as i32..=top).rev().enumerate() {
            if let Some(b) = g.block_at(x, y, area.z, s, min_y) {
                out[row * w + dx as usize] = color(b);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_show_the_ground_and_the_sea() {
        let g = Generator::new(crate::tests::hills());
        let top = top_down(
            &g,
            Area {
                x: 0,
                z: 0,
                w: 64,
                h: 64,
            },
            0,
        );
        assert_eq!(top.len(), 64 * 64);
        assert!(top.iter().all(|c| c >> 24 == 0xff), "every pixel opaque");
        let cut = slice(
            &g,
            Area {
                x: 0,
                z: 0,
                w: 64,
                h: 101,
            },
            100,
            0,
        );
        assert_eq!(cut.len(), 101 * 64);
        assert!(cut[0] == 0, "air above the hills is clear");
        assert_eq!(cut[100 * 64], color("minecraft:bedrock"), "the floor");
        assert_eq!(color("minecraft:grass_block"), 0xff5d8a3a);
        assert_eq!(
            color("mymod:odd"),
            color("mymod:odd"),
            "an unknown block keeps its color"
        );
    }
}
