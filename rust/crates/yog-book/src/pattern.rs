//! Patterns drawn on a hexagonal grid (Hex Casting's): a start direction
//! and a signature of turns — `w` straight on, `e`/`d` 60°/120° right,
//! `q`/`a` 60°/120° left, `s` back — drawn as an SVG.

use serde::{Deserialize, Serialize};

/// One pattern of a [`crate::BookPage::Pattern`] page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HexPattern {
    /// The first stroke's direction: `EAST`, `SOUTH_EAST`, … (or `E`, `SE`, …).
    pub startdir: String,
    /// The turns after it.
    #[serde(default)]
    pub signature: String,
    /// Where it starts on the grid (axial `q`, `r`), when a page shows several.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub q: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub r: i32,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

/// Directions clockwise from east, screen y down.
const DIRS: [(f32, f32); 6] = [
    (1.0, 0.0),
    (0.5, 0.866_025_4),
    (-0.5, 0.866_025_4),
    (-1.0, 0.0),
    (-0.5, -0.866_025_4),
    (0.5, -0.866_025_4),
];

fn dir_index(d: &str) -> Option<usize> {
    Some(match d.trim().to_ascii_uppercase().as_str() {
        "E" | "EAST" => 0,
        "SE" | "SOUTH_EAST" => 1,
        "SW" | "SOUTH_WEST" => 2,
        "W" | "WEST" => 3,
        "NW" | "NORTH_WEST" => 4,
        "NE" | "NORTH_EAST" => 5,
        _ => return None,
    })
}

impl HexPattern {
    /// The points the pattern passes through, in grid units.
    pub fn points(&self) -> Vec<(f32, f32)> {
        let Some(mut dir) = dir_index(&self.startdir) else { return Vec::new() };
        let (q, r) = (self.q as f32, self.r as f32);
        let mut at = (q + r * 0.5, r * 0.866_025_4);
        let mut out = vec![at];
        let mut step = |dir: usize, at: &mut (f32, f32)| {
            at.0 += DIRS[dir].0;
            at.1 += DIRS[dir].1;
            out.push(*at);
        };
        step(dir, &mut at);
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
            dir = (dir + turn) % 6;
            step(dir, &mut at);
        }
        out
    }
}

/// `patterns` as an SVG `w`×`h` (in its own units) — fitted and centred,
/// each a line in `color` (`#rrggbb`) from a dot where it starts.
pub fn svg(patterns: &[HexPattern], w: f32, h: f32, color: &str) -> String {
    let lines: Vec<Vec<(f32, f32)>> = patterns.iter().map(HexPattern::points).filter(|p| p.len() > 1).collect();
    let all = lines.iter().flatten();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in all {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">"#);
    if lines.is_empty() {
        return s + "</svg>";
    }
    // Grid units → the box; a stroke at most a quarter of its width.
    let unit = ((w - 8.0) / (x1 - x0).max(1.0)).min((h - 8.0) / (y1 - y0).max(1.0)).min(w / 4.0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let at = |(x, y): (f32, f32)| (w / 2.0 + (x - cx) * unit, h / 2.0 + (y - cy) * unit);
    let stroke = (unit / 8.0).clamp(0.6, 2.0);
    for line in &lines {
        let pts: Vec<String> = line.iter().map(|&p| {
            let (x, y) = at(p);
            format!("{x:.2},{y:.2}")
        }).collect();
        s += &format!(
            r#"<polyline points="{}" fill="none" stroke="{color}" stroke-width="{stroke:.2}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            pts.join(" ")
        );
        let (x, y) = at(line[0]);
        s += &format!(r#"<circle cx="{x:.2}" cy="{y:.2}" r="{:.2}" fill="{color}"/>"#, stroke * 1.6);
    }
    s + "</svg>"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(dir: &str, sig: &str) -> HexPattern {
        HexPattern { startdir: dir.into(), signature: sig.into(), q: 0, r: 0 }
    }

    #[test]
    fn turns_follow_the_signature() {
        // East, then straight on: three points in a row.
        assert_eq!(p("EAST", "w").points(), [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
        // `s` goes back where it came from.
        let back = p("E", "s").points();
        assert!((back[2].0).abs() < 1e-5 && (back[2].1).abs() < 1e-5);
        // `qaq` (Mind's Reflection): five points.
        assert_eq!(p("NORTH_EAST", "qaq").points().len(), 5);
        assert!(p("UP", "w").points().is_empty(), "an unknown direction draws nothing");
    }

    #[test]
    fn svg_has_a_line_per_pattern() {
        let s = svg(&[p("NE", "qaq"), HexPattern { q: 2, ..p("E", "qaq") }], 100.0, 60.0, "#8d6acc");
        assert_eq!(s.matches("<polyline").count(), 2);
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
    }
}
