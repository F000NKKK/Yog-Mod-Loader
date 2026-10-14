//! Patterns on a hexagonal grid (Hex Casting's), drawn as its book draws
//! them: a start direction and a signature of turns — `w` straight on,
//! `e`/`d` 60°/120° right, `q`/`a` 60°/120° left, `s` back — as a "zappy"
//! line (each stroke broken into little noisy hops, the same every time),
//! dark inside a light outline, a blue dot where it starts, faint dots on
//! the points it passes.

use serde::{Deserialize, Serialize};

/// One pattern of a [`crate::BookPage::Pattern`] page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, yog_data::Schema)]
pub struct HexPattern {
    /// The first stroke's direction: `EAST`, `SOUTH_EAST`, … (or `E`, `SE`, …).
    pub startdir: String,
    /// The turns after it.
    #[serde(default)]
    #[data(default)]
    pub signature: String,
}

type V = (f32, f32);

/// Directions clockwise from north-east, as axial `(q, r)` steps.
const DIRS: [(i32, i32); 6] = [(1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1)];

fn dir_index(d: &str) -> Option<usize> {
    Some(match d.trim().to_ascii_uppercase().as_str() {
        "NE" | "NORTH_EAST" => 0,
        "E" | "EAST" => 1,
        "SE" | "SOUTH_EAST" => 2,
        "SW" | "SOUTH_WEST" => 3,
        "W" | "WEST" => 4,
        "NW" | "NORTH_WEST" => 5,
        _ => return None,
    })
}

impl HexPattern {
    /// The grid points it passes through, in order (axial `q`, `r`).
    pub fn positions(&self) -> Vec<(i32, i32)> {
        let Some(mut dir) = dir_index(&self.startdir) else {
            return Vec::new();
        };
        let mut at = (0, 0);
        let mut out = vec![at];
        for c in self.signature.chars() {
            let turn = match c {
                'w' => 0,
                'e' => 1,
                'd' => 2,
                's' => 3,
                'a' => 4,
                'q' => 5,
                _ => continue,
            };
            at = (at.0 + DIRS[dir].0, at.1 + DIRS[dir].1);
            out.push(at);
            dir = (dir + turn) % 6;
        }
        out.push((at.0 + DIRS[dir].0, at.1 + DIRS[dir].1));
        out
    }
}

/// A grid point on screen (unit hexes; y down).
fn to_px((q, r): (i32, i32)) -> V {
    let s3 = 3f32.sqrt();
    (s3 * q as f32 + s3 / 2.0 * r as f32, 1.5 * r as f32)
}

/// Indices of the points visited more than once.
fn dup_indices(pos: &[(i32, i32)]) -> Vec<bool> {
    let mut first = std::collections::HashMap::new();
    let mut dup = vec![false; pos.len()];
    for (i, p) in pos.iter().enumerate() {
        match first.get(p) {
            Some(&j) => {
                dup[i] = true;
                dup[j] = true;
            }
            None => {
                first.insert(*p, i);
            }
        }
    }
    dup
}

// ── Minecraft's noise and trigonometry, as Hex Casting draws with them ──

/// `SingleThreadedRandomSource`: Java's LCG.
struct Lcg(i64);

impl Lcg {
    fn new(seed: i64) -> Lcg {
        Lcg((seed ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }
    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(11)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }
    fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let i = self.next(31);
            let j = i % bound;
            if i.wrapping_sub(j).wrapping_add(bound - 1) >= 0 {
                return j;
            }
        }
    }
    fn next_double(&mut self) {
        self.next(26);
        self.next(27);
    }
}

/// `SimplexNoise` (3D).
struct Simplex([i32; 256]);

const GRADIENT: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

impl Simplex {
    fn new(rng: &mut Lcg) -> Simplex {
        // (Its offsets, unused in 3D, still take their turn of the random.)
        for _ in 0..3 {
            rng.next_double();
        }
        let mut p = [0i32; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as i32;
        }
        for l in 0..256 {
            let j = rng.next_int(256 - l as i32) as usize;
            p.swap(l, j + l);
        }
        Simplex(p)
    }
    fn p(&self, i: i32) -> i32 {
        self.0[(i & 255) as usize]
    }
    fn corner(g: i32, x: f64, y: f64, z: f64) -> f64 {
        let d = 0.6 - x * x - y * y - z * z;
        if d < 0.0 {
            return 0.0;
        }
        let d = d * d;
        let g = GRADIENT[g as usize];
        d * d * (g[0] as f64 * x + g[1] as f64 * y + g[2] as f64 * z)
    }
    fn value(&self, x: f64, y: f64, z: f64) -> f64 {
        let s = (x + y + z) * 0.333_333_333_333_333_3;
        let (i, j, k) = (
            (x + s).floor() as i32,
            (y + s).floor() as i32,
            (z + s).floor() as i32,
        );
        let t = (i + j + k) as f64 * 0.166_666_666_666_666_66;
        let (x0, y0, z0) = (x - (i as f64 - t), y - (j as f64 - t), z - (k as f64 - t));
        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };
        let g6 = 0.166_666_666_666_666_66;
        let g3 = 0.333_333_333_333_333_3;
        let (x1, y1, z1) = (
            x0 - i1 as f64 + g6,
            y0 - j1 as f64 + g6,
            z0 - k1 as f64 + g6,
        );
        let (x2, y2, z2) = (
            x0 - i2 as f64 + g3,
            y0 - j2 as f64 + g3,
            z0 - k2 as f64 + g3,
        );
        let (x3, y3, z3) = (x0 - 0.5, y0 - 0.5, z0 - 0.5);
        let (ii, jj, kk) = (i & 255, j & 255, k & 255);
        let g0 = self.p(ii + self.p(jj + self.p(kk))) % 12;
        let g1 = self.p(ii + i1 + self.p(jj + j1 + self.p(kk + k1))) % 12;
        let g2 = self.p(ii + i2 + self.p(jj + j2 + self.p(kk + k2))) % 12;
        let g3i = self.p(ii + 1 + self.p(jj + 1 + self.p(kk + 1))) % 12;
        32.0 * (Self::corner(g0, x0, y0, z0)
            + Self::corner(g1, x1, y1, z1)
            + Self::corner(g2, x2, y2, z2)
            + Self::corner(g3i, x3, y3, z3))
    }
}

fn noise() -> &'static Simplex {
    static N: std::sync::OnceLock<Simplex> = std::sync::OnceLock::new();
    N.get_or_init(|| Simplex::new(&mut Lcg::new(9001)))
}

/// Hex Casting's `getNoise`.
fn get_noise(x: f64, y: f64, z: f64) -> f64 {
    noise().value(x * 0.6, y * 0.6, z * 0.6) / 2.0
}

/// `Mth.sin`/`Mth.cos`: a 65536-step table.
fn mth_sin(f: f32) -> f32 {
    let i = ((f * 10430.378) as i32) & 0xFFFF;
    (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32
}
fn mth_cos(f: f32) -> f32 {
    let i = ((f * 10430.378 + 16384.0) as i32) & 0xFFFF;
    (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32
}

/// Hex Casting's `makeZappy` (at rest: its book's patterns don't move).
fn make_zappy(
    points: &[V],
    dups: &[bool],
    readability: f32,
    last_segment: f32,
    seed: f64,
) -> Vec<V> {
    const HOPS: usize = 10;
    const VARIANCE: f32 = 0.5;
    const FLOW_IRREGULAR: f64 = 0.2;
    let zappify = |pts: &[V], truncate_last: bool| -> Vec<V> {
        let scale_variance = |t: f64| 1f64.min(8.0 * (0.5 - (0.5 - t).abs()));
        let mut out = vec![pts[0]];
        for i in 0..pts.len().saturating_sub(1) {
            let (src, target) = (pts[i], pts[i + 1]);
            let delta = (target.0 - src.0, target.1 - src.1);
            let hop_dist =
                ((target.0 - src.0).powi(2) + (target.1 - src.1).powi(2)).sqrt() / HOPS as f32;
            let max_variance = hop_dist * VARIANCE;
            let max_j = if truncate_last && i == pts.len() - 2 {
                (last_segment * HOPS as f32).round() as usize
            } else {
                HOPS
            };
            for j in 1..=max_j {
                let progress = j as f64 / (HOPS + 1) as f64;
                let pos = (
                    src.0 + delta.0 * progress as f32,
                    src.1 + delta.1 * progress as f32,
                );
                let minor = get_noise(i as f64, j as f64, 0.0) * FLOW_IRREGULAR;
                let theta = (3.0
                    * get_noise(i as f64 + progress + minor, 1337.0, seed)
                    * std::f64::consts::TAU) as f32;
                let r = (get_noise(i as f64 + progress, 69420.0, seed)
                    * max_variance as f64
                    * scale_variance(progress)) as f32;
                out.push((pos.0 + r * mth_cos(theta), pos.1 + r * mth_sin(theta)));
                if j == HOPS {
                    out.push(target);
                }
            }
        }
        out
    };
    let mut out = Vec::new();
    let mut chain: Vec<V> = Vec::new();
    for i in 0..points.len().saturating_sub(1) {
        let (head, tail) = (points[i], points[i + 1]);
        let tangent = (
            (tail.0 - head.0) * readability,
            (tail.1 - head.1) * readability,
        );
        chain.push(if i != 0 && dups[i] {
            (head.0 + tangent.0, head.1 + tangent.1)
        } else {
            head
        });
        if i == points.len() - 2 {
            chain.push(tail);
            out.extend(zappify(&chain, true));
        } else if dups[i + 1] {
            chain.push((tail.0 - tangent.0, tail.1 - tangent.1));
            out.extend(zappify(&chain, false));
            chain.clear();
        }
    }
    out
}

/// Hex Casting's book colors (`READABLE_GRID_SCROLL_COLORS`) and strokes (`fromStroke(4)`).
const OUTER: &str = "#d2c8c8";
const INNER: &str = "#554d54";
const START_DOT: &str = "#5b7bd7";
const STROKE: f64 = 4.0;

/// `patterns` as an SVG `w`×`h` (in book pixels), as Hex Casting's book
/// lays them out: a grid of cells, one pattern centred in each.
pub fn svg(patterns: &[HexPattern], w: f32, h: f32) -> String {
    let (w, h) = (w as f64, h as f64);
    let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">"#);
    let n = patterns.len().max(1);
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let (cell_w, cell_h) = (w / cols as f64, h / rows as f64);
    for (k, pat) in patterns.iter().enumerate() {
        let pos = pat.positions();
        if pos.len() < 2 {
            continue;
        }
        let dots: Vec<V> = pos.iter().map(|&p| to_px(p)).collect();
        let zappy = make_zappy(&dots, &dup_indices(&pos), 0.2, 0.8, 0.0);
        // HexPatternPoints: fit into the cell (padding 2, base scale 16).
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in &zappy {
            x0 = x0.min(x as f64);
            x1 = x1.max(x as f64);
            y0 = y0.min(y as f64);
            y1 = y1.max(y as f64);
        }
        let base = 16.0 / 1.5;
        let (bw, bh) = ((x1 - x0) * base, (y1 - y0) * base);
        let mut scale = 1f64;
        scale = scale.min((cell_h - 4.0 - STROKE) / bh);
        scale = scale.min((cell_w - 4.0 - STROKE) / bw);
        let fin = base * scale;
        let (iw, ih) = (bw * scale + 4.0 + STROKE, bh * scale + 4.0 + STROKE);
        let off_x = (iw - bw * scale) / 2.0 + (cell_w - iw).max(0.0) / 2.0;
        let off_y = (ih - bh * scale) / 2.0 + (cell_h - ih).max(0.0) / 2.0;
        let (cx, cy) = (cell_w * (k % cols) as f64, cell_h * (k / cols) as f64);
        let at = |&(x, y): &V| {
            (
                cx + (x as f64 - x0) * fin + off_x,
                cy + (y as f64 - y0) * fin + off_y,
            )
        };
        let line: Vec<String> = zappy
            .iter()
            .map(|p| {
                let (x, y) = at(p);
                format!("{x:.2},{y:.2}")
            })
            .collect();
        let line = line.join(" ");
        for (color, width) in [(OUTER, STROKE), (INNER, STROKE * 2.0 / 5.0)] {
            s += &format!(
                r#"<polyline points="{line}" fill="none" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>"#
            );
        }
        // Dots: little hexagons.
        let spot = |(x, y): (f64, f64), r: f64, fill: &str, opacity: f64| {
            let pts: Vec<String> = (0..6)
                .map(|i| {
                    let t = i as f64 / 6.0 * std::f64::consts::TAU;
                    format!("{:.2},{:.2}", x + t.cos() * r, y + t.sin() * r)
                })
                .collect();
            format!(
                r#"<polygon points="{}" fill="{fill}" fill-opacity="{opacity}"/>"#,
                pts.join(" ")
            )
        };
        s += &spot(at(&dots[0]), 0.8 * STROKE * 2.0 / 5.0, START_DOT, 1.0);
        for d in &dots[1..] {
            s += &spot(at(d), 0.4 * STROKE * 2.0 / 5.0, OUTER, 0.5);
        }
    }
    s + "</svg>"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(dir: &str, sig: &str) -> HexPattern {
        HexPattern {
            startdir: dir.into(),
            signature: sig.into(),
        }
    }

    #[test]
    fn positions_follow_the_signature() {
        // East, then straight on: three points in a row.
        assert_eq!(p("EAST", "w").positions(), [(0, 0), (1, 0), (2, 0)]);
        // `s` goes back where it came from.
        assert_eq!(p("E", "s").positions(), [(0, 0), (1, 0), (0, 0)]);
        // `qaq` (Mind's Reflection): five points.
        assert_eq!(p("NORTH_EAST", "qaq").positions().len(), 5);
        assert!(
            p("UP", "w").positions().is_empty(),
            "an unknown direction draws nothing"
        );
    }

    #[test]
    fn random_and_noise_match_minecraft() {
        // The same LCG as java.util.Random: `new Random(0).nextInt()`.
        assert_eq!(Lcg::new(0).next(32), -1155484576);
        // The noise is smooth and in range.
        let a = get_noise(0.5, 1337.0, 0.0);
        assert!(a.abs() <= 0.5 && (a - get_noise(0.5001, 1337.0, 0.0)).abs() < 1e-3);
        assert_eq!(mth_sin(0.0), 0.0);
        assert!((mth_cos(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zappy_lines_stop_short_and_stay_close() {
        let pos = p("E", "w").positions();
        let dots: Vec<V> = pos.iter().map(|&q| to_px(q)).collect();
        let z = make_zappy(&dots, &dup_indices(&pos), 0.2, 0.8, 0.0);
        // 1 + 10 hops + the target, then 8 hops of the last stroke.
        assert_eq!(z.len(), 1 + 10 + 1 + 8);
        assert!(
            z.iter()
                .all(|&(x, y)| (-0.5..=3.7).contains(&x) && y.abs() < 0.5)
        );
    }

    #[test]
    fn svg_has_a_line_per_pattern() {
        let s = svg(&[p("NE", "qaq"), p("E", "qaq")], 116.0, 64.0);
        assert_eq!(s.matches("<polyline").count(), 4, "outline and line each");
        assert!(s.contains(START_DOT));
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
    }
}
