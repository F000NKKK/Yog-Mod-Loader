//! Pictures of a terrain for the IDE: from above (the top block's color,
//! shaded by slope) and a slice down through it — from the generator the
//! game runs.

use crate::Generator;

/// An `0xAARRGGBB` color for a block: the common ones as they look, any
/// other a steady color of its own.
pub fn color(block: &str) -> u32 {
    let path = block.split_once(':').map_or(block, |(_, p)| p);
    let known = match path {
        "grass_block" | "moss_block" => 0x5d8a3a,
        "dirt" | "coarse_dirt" | "rooted_dirt" => 0x866043,
        "podzol" => 0x5a3e1d,
        "stone" | "cobblestone" | "andesite" => 0x7d7d7d,
        "deepslate" => 0x4d4d50,
        "granite" => 0x9a6a55,
        "diorite" => 0xbcbcbc,
        "sand" => 0xdbd3a0,
        "red_sand" => 0xbe6621,
        "sandstone" => 0xd8cb9b,
        "gravel" => 0x837e7d,
        "clay" => 0x9ea4b0,
        "snow_block" | "snow" | "powder_snow" => 0xf3f6f6,
        "ice" | "packed_ice" | "blue_ice" => 0x93b4ec,
        "water" => 0x3f5fd5,
        "lava" => 0xd65a12,
        "netherrack" => 0x6f3534,
        "soul_sand" | "soul_soil" => 0x4f3b2f,
        "basalt" | "blackstone" => 0x3c3a3e,
        "end_stone" => 0xdcdf9e,
        "obsidian" => 0x150f22,
        "bedrock" => 0x333333,
        "mycelium" => 0x6f6265,
        "terracotta" => 0x985e43,
        "calcite" => 0xdfe0dc,
        "tuff" => 0x6c6d66,
        "mud" => 0x3c3a3d,
        p if p.ends_with("_ore") => 0x8a8a8a,
        p if p.ends_with("_log") || p.ends_with("_wood") => 0x6b5130,
        p if p.ends_with("_leaves") => 0x3e7a2b,
        _ => {
            // FNV-1a of the id: some color, the same every time.
            let mut h: u32 = 0x811c_9dc5;
            for b in block.bytes() {
                h = (h ^ b as u32).wrapping_mul(0x0100_0193);
            }
            h & 0x00ff_ffff
        }
    };
    0xff00_0000 | known
}

fn shade(c: u32, f: f64) -> u32 {
    let ch = |s: u32| (((c >> s) & 0xff) as f64 * f).clamp(0.0, 255.0) as u32;
    0xff00_0000 | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// The terrain from above: `w`×`h` pixels, one a block from `x0`, `z0`;
/// rows top to bottom (north to south).
pub fn top_down(g: &Generator, x0: i32, z0: i32, w: u32, h: u32, min_y: i32) -> Vec<u32> {
    let mut out = Vec::with_capacity((w * h) as usize);
    for dz in 0..h as i32 {
        for dx in 0..w as i32 {
            let (x, z) = (x0 + dx, z0 + dz);
            let s = g.surface(x, z);
            // The highest block that isn't air: a fluid over low ground, else the ground's top.
            let (block, fluid) = match &g.terrain.fluid {
                Some(f) if f.level > s => (f.block.as_str(), true),
                _ => (g.block_at(x, s, z, s, min_y).unwrap_or(&g.terrain.fill), false),
            };
            let c = color(block);
            // Light from the north-west: brighter where the ground rises toward it.
            let slope = if fluid { 0.0 } else { (s - g.surface(x - 1, z - 1)) as f64 };
            out.push(shade(c, (1.0 + slope * 0.08).clamp(0.6, 1.3)));
        }
    }
    out
}

/// A slice down through the terrain along x at `z`: `w` blocks from `x0`,
/// from `top` down to `bottom`; rows top to bottom. Air is transparent.
pub fn slice(g: &Generator, x0: i32, z: i32, w: u32, top: i32, bottom: i32, min_y: i32) -> Vec<u32> {
    let rows = (top - bottom + 1).max(0) as usize;
    let mut out = vec![0u32; rows * w as usize];
    for dx in 0..w as i32 {
        let x = x0 + dx;
        let s = g.surface(x, z);
        for (row, y) in (bottom..=top).rev().enumerate() {
            if let Some(b) = g.block_at(x, y, z, s, min_y) {
                out[row * w as usize + dx as usize] = color(b);
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
        let top = top_down(&g, 0, 0, 64, 64, 0);
        assert_eq!(top.len(), 64 * 64);
        assert!(top.iter().all(|c| c >> 24 == 0xff), "every pixel opaque");
        let cut = slice(&g, 0, 0, 64, 100, 0, 0);
        assert_eq!(cut.len(), 101 * 64);
        assert!(cut[0] == 0, "air above the hills is clear");
        assert_eq!(cut[100 * 64], color("minecraft:bedrock"), "the floor");
        assert_eq!(color("minecraft:grass_block"), 0xff5d8a3a);
        assert_eq!(color("mymod:odd"), color("mymod:odd"), "an unknown block keeps its color");
    }
}
