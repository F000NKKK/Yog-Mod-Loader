//! A tab of the game's advancements screen, drawn as the game draws it
//! (`AdvancementsScreen`, `AdvancementTab`, `AdvancementWidget`,
//! `TreeNodePosition` of 1.20.1–1.21.1): the tree laid out by the game's
//! algorithm, the window, the root's background, the connectors, the frames
//! and icons, and a hovered advancement's title bar and description — in
//! the game's font, in [`SCREEN`] GUI pixels around the window.

use std::path::Path;

use base64::Engine as _;
use yog_gfx::draw2d::Draw2D;
use yog_gfx::{Color, Rect, Vec2};

use super::assets::Resources;
mod args;

pub use args::{AdvancementArgs, NodeArgs};

use super::{gfx, images, raster};

/// The window, and its tab's view inside it.
const WINDOW: (f32, f32) = (252.0, 140.0);
/// Around the window: room for the backdrop to show and a hover's box to turn.
const MARGIN: f32 = 8.0;
/// What's drawn: the window and its margin, as big as the editor makes it.
const SCREEN: (f32, f32) = (WINDOW.0 + 2.0 * MARGIN, WINDOW.1 + 2.0 * MARGIN);
const VIEW: Rect = Rect {
    x: 9.0,
    y: 18.0,
    w: 234.0,
    h: 113.0,
};
/// Where the tab's title is written.
const TITLE_AT: Vec2 = Vec2 { x: 8.0, y: 6.0 };
/// A tree position's step (column, row) and a widget's frame.
const STEP: (f32, f32) = (28.0, 27.0);
const FRAME: f32 = 26.0;
const FRAME_DX: f32 = 3.0;
const ICON_AT: Vec2 = Vec2 { x: 8.0, y: 5.0 };
const ICON: f32 = 16.0;
/// A background tile's side.
const TILE: f32 = 16.0;
/// A connector: where it leaves a frame, how far it runs before turning.
const CONNECT_MID: f32 = 13.0;
const CONNECT_OUT: f32 = FRAME + 4.0;
/// The hover: the bar's sprite width, the widest title, the text's place
/// and line height, the room it leaves before the screen's edge.
const BAR_W: f32 = 200.0;
const TITLE_MAX: f32 = 163.0;
const BAR_TEXT: Vec2 = Vec2 { x: 32.0, y: 9.0 };
const TEXT_PAD_LEFT: f32 = 3.0;
const TEXT_PAD_RIGHT: f32 = 5.0;
const TEXT_LEFT: f32 = 5.0;
const LINE_H: f32 = 9.0;
const TITLE_BAR_MIN: f32 = 29.0;
const DESCRIPTION_TOP: f32 = 17.0;
/// The description's first line from the top of a box that opens upward.
const DESCRIPTION_TOP_UP: f32 = 7.0;
const DESCRIPTION_PAD: f32 = 32.0;
const FLIP_ROOM: f32 = 6.0;
/// How far below or above the target width the game tries a description's
/// line breaks, and how close is close enough.
const SPLIT_OFFSETS: [f32; 5] = [0.0, 10.0, -10.0, 25.0, -25.0];
const SPLIT_GOOD_ENOUGH: f32 = 10.0;
/// What a hover dims the view with (`AdvancementTab`'s fade at its most).
const HOVER_FADE: Color = Color(0x4C00_0000);
/// The screen's dimmed world behind the window (`renderBackground`).
const BACKDROP: (Color, Color) = (Color(0xC010_1010), Color(0xD010_1010));
const TITLE_COLOR: Color = Color(0xFF40_4040);
const WHITE: Color = Color(0xFFFF_FFFF);
const BLACK: Color = Color(0xFF00_0000);
/// How much bigger than GUI pixels the picture is drawn.
const SCALE: u32 = 1;

/// One advancement of the tab, its texts as shown.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub parent: Option<String>,
    pub icon: Option<String>,
    /// `task`, `goal` or `challenge`.
    pub frame: String,
    pub title: String,
    pub description: String,
    /// Drawn as had (the one being edited).
    pub obtained: bool,
}

/// What to draw, its texts as shown.
#[derive(Debug, Clone)]
pub struct Args {
    pub path: String,
    pub title: String,
    /// The root's background texture (`minecraft:textures/block/calcite.png` or `minecraft:block/calcite`).
    pub background: Option<String>,
    pub nodes: Vec<Node>,
    /// How far the reader dragged the tree, GUI pixels.
    pub drag: [f32; 2],
    /// The advancement under the pointer.
    pub hover: Option<String>,
}

/// Where an advancement can be pointed at, in screen GUI pixels.
#[derive(Debug, Clone, PartialEq, yog_data::Schema)]
pub struct Spot {
    pub id: String,
    pub rect: [f32; 4],
}

/// The drawn screen.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Rendered {
    pub image: Option<String>,
    pub size: [f32; 2],
    /// The view (where dragging scrolls), and each advancement's spot.
    pub view: [f32; 4],
    pub spots: Vec<Spot>,
    /// The drag the tab accepts (clamped as the game clamps its scroll).
    pub drag: [f32; 2],
}

/// The tab laid out: each shown advancement (its node's index, spot, its
/// parent's spot), and where the window, its view and the scrolled tree are.
struct Tab {
    spots: Vec<(usize, Vec2, Option<Vec2>)>,
    scroll: Vec2,
    window: Vec2,
    view: Vec2,
    origin: Vec2,
}

impl Tab {
    fn view_rect(&self) -> Rect {
        Rect::new(self.view.x, self.view.y, VIEW.w, VIEW.h)
    }
}

/// Draws the tab.
pub fn render(args: &Args) -> Rendered {
    let root = super::mod_root(Path::new(&args.path));
    images::set_document(root.as_deref());
    let res = Resources::for_mod(root.as_deref());
    let (tab, drag) = lay_out(args);
    let ops = gfx::record(SCREEN.0 as i32, SCREEN.1 as i32, |ctx| {
        draw(&ctx.draw2d(), args, &tab)
    });
    let canvas = raster::rasterize(&ops, SCREEN.0 as u32, SCREEN.1 as u32, SCALE, &res);
    let image = canvas.png().ok().map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    let o = tab.origin;
    Rendered {
        image,
        size: [SCREEN.0, SCREEN.1],
        view: [tab.view.x, tab.view.y, VIEW.w, VIEW.h],
        spots: tab
            .spots
            .iter()
            .map(|(i, p, _)| Spot {
                id: args.nodes[*i].id.clone(),
                rect: [o.x + p.x, o.y + p.y, FRAME + 1.0, FRAME + 1.0],
            })
            .collect(),
        drag,
    }
}

/// Where everything goes: the spots, and the tab's extent and its scroll
/// (`AdvancementTab.addWidget`, `drawContents`, `scroll`); the drag accepted.
fn lay_out(args: &Args) -> (Tab, [f32; 2]) {
    let at = layout(&args.nodes);
    let place = |id: &str| {
        at.iter()
            .find(|(i, _)| i == id)
            .map(|(_, p)| Vec2::new((p.0 * STEP.0).floor(), (p.1 * STEP.1).floor()))
    };
    let spots: Vec<(usize, Vec2, Option<Vec2>)> = args
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| Some((i, place(&n.id)?, n.parent.as_deref().and_then(place))))
        .collect();
    let min_x = spots.iter().map(|(_, p, _)| p.x).fold(f32::MAX, f32::min);
    let max_x = spots
        .iter()
        .map(|(_, p, _)| p.x + STEP.0)
        .fold(f32::MIN, f32::max);
    let min_y = spots.iter().map(|(_, p, _)| p.y).fold(f32::MAX, f32::min);
    let max_y = spots
        .iter()
        .map(|(_, p, _)| p.y + STEP.1)
        .fold(f32::MIN, f32::max);
    let centre = Vec2::new(
        (VIEW.w / 2.0 - ((max_x + min_x) / 2.0).trunc()).trunc(),
        ((VIEW.h / 2.0).floor() - ((max_y + min_y) / 2.0).trunc()).trunc(),
    );
    let axis = |centre: f32, drag: f32, extent: f32, min: f32, view: f32| {
        if extent - min > view {
            (centre + drag).clamp(-(extent - view), 0.0) - centre
        } else {
            0.0
        }
    };
    let drag = [
        axis(centre.x, args.drag[0], max_x, min_x, VIEW.w),
        axis(centre.y, args.drag[1], max_y, min_y, VIEW.h),
    ];
    let scroll = Vec2::new((centre.x + drag[0]).floor(), (centre.y + drag[1]).floor());
    let window = Vec2::new(
        ((SCREEN.0 - WINDOW.0) / 2.0).floor(),
        ((SCREEN.1 - WINDOW.1) / 2.0).floor(),
    );
    let view = Vec2::new(window.x + VIEW.x, window.y + VIEW.y);
    let origin = Vec2::new(view.x + scroll.x, view.y + scroll.y);
    (
        Tab {
            spots,
            scroll,
            window,
            view,
            origin,
        },
        drag,
    )
}

/// The screen: the world behind dimmed, the view, the window, the title,
/// the hover.
fn draw(d: &Draw2D, args: &Args, tab: &Tab) {
    // The world behind, dimmed (here: nothing behind).
    d.rect(Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1), BLACK);
    d.gradient(
        Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1),
        BACKDROP.0,
        BACKDROP.1,
    );
    draw_background(d, args, tab);
    draw_tree(d, args, tab);
    let w = tab.window;
    d.image(WINDOW_SPRITE, Rect::new(w.x, w.y, WINDOW.0, WINDOW.1));
    d.text(
        &args.title,
        Vec2::new(w.x + TITLE_AT.x, w.y + TITLE_AT.y),
        TITLE_COLOR,
        false,
    );
    // The hover (`AdvancementTab.drawTooltips`, `AdvancementWidget.drawHover`).
    if let Some((i, p, _)) = args
        .hover
        .as_deref()
        .and_then(|h| tab.spots.iter().find(|(i, _, _)| args.nodes[*i].id == h))
    {
        d.rect(tab.view_rect(), HOVER_FADE);
        hover(d, &args.nodes[*i], tab.origin, *p, w);
    }
}

/// The view's background, tiled from the scroll (clipped by the view).
fn draw_background(d: &Draw2D, args: &Args, tab: &Tab) {
    let background = args.background.as_deref().map_or_else(
        || yog_ids::game::MISSING_TEXTURE.to_string(),
        yog_gfx::image::short_source,
    );
    let (ox, oy) = (tab.scroll.x % TILE, tab.scroll.y % TILE);
    let view = tab.view;
    for i in -1..=(VIEW.w / TILE) as i32 {
        for j in -1..=(VIEW.h / TILE) as i32 {
            let tile = Rect::new(
                view.x + ox + TILE * i as f32,
                view.y + oy + TILE * j as f32,
                TILE,
                TILE,
            );
            if let Some(t) = clip(tile, tab.view_rect()) {
                let uv = Rect::new(t.x - tile.x, t.y - tile.y, t.w, t.h);
                d.image_region(&background, t, Some(uv), WHITE);
            }
        }
    }
}

/// The connectors (their shadow first), then each advancement's frame and icon.
fn draw_tree(d: &Draw2D, args: &Args, tab: &Tab) {
    let view_rect = tab.view_rect();
    let fill = |r: Rect, c: Color| {
        if let Some(r) = clip(r, view_rect) {
            d.rect(r, c);
        }
    };
    for shadow in [true, false] {
        for (_, p, parent) in &tab.spots {
            if let Some(pp) = parent {
                connector(&fill, tab.origin, *pp, *p, shadow);
            }
        }
    }
    let o = tab.origin;
    for (i, p, _) in &tab.spots {
        let n = &args.nodes[*i];
        let frame = Rect::new(o.x + p.x + FRAME_DX, o.y + p.y, FRAME, FRAME);
        if clip(frame, view_rect).is_some() {
            d.image(&frame_sprite(&n.frame, n.obtained), frame);
            if let Some(icon) = &n.icon {
                d.item(
                    icon,
                    Vec2::new(o.x + p.x + ICON_AT.x, o.y + p.y + ICON_AT.y),
                    ICON,
                );
            }
        }
    }
}

/// The window's sprite and a frame's (`yog-gfx/assets/sprites.ydata`).
const WINDOW_SPRITE: &str = "sprite:advancement_window";
fn frame_sprite(frame: &str, obtained: bool) -> String {
    format!(
        "sprite:advancement/{frame}{}",
        if obtained { "_obtained" } else { "" }
    )
}
fn bar_sprite(obtained: bool) -> &'static str {
    if obtained {
        "sprite:advancement/box_obtained"
    } else {
        "sprite:advancement/box"
    }
}
const DESCRIPTION_SPRITE: &str = "sprite:advancement/description";

/// The frame that's a challenge, and the chat colours `FrameType` writes descriptions in.
const CHALLENGE: &str = "challenge";
const DARK_PURPLE: char = '5';
const GREEN: char = 'a';

/// The colour a frame's description is written in (`FrameType`'s chat colour).
fn description_color(frame: &str) -> Color {
    let code = if frame == CHALLENGE {
        DARK_PURPLE
    } else {
        GREEN
    };
    Color(super::font::chat_color(code).unwrap_or(Color::WHITE.0))
}

/// `AdvancementWidget.drawConnectivity`: `GuiGraphics.hLine` / `vLine` rectangles.
fn connector(fill: &impl Fn(Rect, Color), o: Vec2, parent: Vec2, child: Vec2, shadow: bool) {
    let (i, j, k) = (
        o.x + parent.x + CONNECT_MID,
        o.x + parent.x + CONNECT_OUT,
        o.y + parent.y + CONNECT_MID,
    );
    let (l, m) = (o.x + child.x + CONNECT_MID, o.y + child.y + CONNECT_MID);
    let h = |a: f32, b: f32, y: f32, c: Color| {
        fill(Rect::new(a.min(b), y, (a - b).abs() + 1.0, 1.0), c)
    };
    let v = |x: f32, a: f32, b: f32, c: Color| {
        fill(
            Rect::new(x, a.min(b) + 1.0, 1.0, ((a - b).abs() - 1.0).max(0.0)),
            c,
        )
    };
    if shadow {
        h(j, i, k - 1.0, BLACK);
        h(j + 1.0, i, k, BLACK);
        h(j, i, k + 1.0, BLACK);
        h(l, j - 1.0, m - 1.0, BLACK);
        h(l, j - 1.0, m, BLACK);
        h(l, j - 1.0, m + 1.0, BLACK);
        v(j - 1.0, m, k, BLACK);
        v(j + 1.0, m, k, BLACK);
    } else {
        h(j, i, k, WHITE);
        h(l, j, m, WHITE);
        v(j, m, k, WHITE);
    }
}

/// `AdvancementWidget.drawHover`, an advancement had or not (no progress between).
fn hover(d: &yog_gfx::draw2d::Draw2D<'_>, n: &Node, o: Vec2, p: Vec2, window: Vec2) {
    let width_of = |s: &str| d.text_width(s, "");
    let title = cut_to_width(&n.title, TITLE_MAX, &width_of);
    let mut inner = TITLE_BAR_MIN + width_of(&title);
    let lines = optimal_lines(&n.description, inner, &width_of);
    for l in &lines {
        inner = inner.max(width_of(l));
    }
    let width = inner + TEXT_PAD_LEFT + TEXT_PAD_RIGHT;
    let y = o.y + p.y;
    // Flipped left when it would run off the screen; the description above when below has no room.
    let flip = o.x + p.x + width + FRAME >= SCREEN.0;
    let up =
        VIEW.h - (o.y - window.y - VIEW.y) - p.y - FRAME <= FLIP_ROOM + lines.len() as f32 * LINE_H;
    let x = if flip {
        o.x + p.x - width + FRAME + FLIP_ROOM
    } else {
        o.x + p.x
    };
    let box_h = DESCRIPTION_PAD + lines.len() as f32 * LINE_H;
    if !lines.is_empty() {
        let top = if up { y + FRAME - box_h } else { y };
        d.image(DESCRIPTION_SPRITE, Rect::new(x, top, width, box_h));
    }
    let half = (width / 2.0).floor();
    let rest = width - half;
    d.image_region(
        bar_sprite(n.obtained),
        Rect::new(x, y, half, FRAME),
        Some(Rect::new(0.0, 0.0, half, FRAME)),
        WHITE,
    );
    d.image_region(
        bar_sprite(n.obtained),
        Rect::new(x + half, y, rest, FRAME),
        Some(Rect::new(BAR_W - rest, 0.0, rest, FRAME)),
        WHITE,
    );
    d.image(
        &frame_sprite(&n.frame, n.obtained),
        Rect::new(o.x + p.x + FRAME_DX, y, FRAME, FRAME),
    );
    let title_x = if flip {
        x + TEXT_LEFT
    } else {
        o.x + p.x + BAR_TEXT.x
    };
    d.text(&title, Vec2::new(title_x, y + BAR_TEXT.y), WHITE, true);
    let color = description_color(&n.frame);
    for (i, line) in lines.iter().enumerate() {
        let ly = if up {
            y + FRAME - box_h + DESCRIPTION_TOP_UP + i as f32 * LINE_H
        } else {
            y + BAR_TEXT.y + DESCRIPTION_TOP + i as f32 * LINE_H
        };
        d.text(line, Vec2::new(x + TEXT_LEFT, ly), color, false);
    }
    if let Some(icon) = &n.icon {
        d.item(icon, Vec2::new(o.x + p.x + ICON_AT.x, y + ICON_AT.y), ICON);
    }
}

/// `Font.substrByWidth`: the longest start of `s` no wider than `max`.
fn cut_to_width(s: &str, max: f32, width: &impl Fn(&str) -> f32) -> String {
    let mut end = s.len();
    while end > 0 && width(&s[..end]) > max {
        end = s[..end].char_indices().last().map_or(0, |(i, _)| i);
    }
    s[..end].to_string()
}

/// `AdvancementWidget.findOptimalLines`: the breaks whose widest line is
/// nearest `target`, trying a few widths around it.
fn optimal_lines(text: &str, target: f32, width: &impl Fn(&str) -> f32) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut best: Option<(f32, Vec<String>)> = None;
    for off in SPLIT_OFFSETS {
        let lines = split_lines(text, target - off, width);
        let gap = (lines.iter().map(|l| width(l)).fold(0.0, f32::max) - target).abs();
        if gap <= SPLIT_GOOD_ENOUGH {
            return lines;
        }
        if best.as_ref().is_none_or(|(g, _)| gap < *g) {
            best = Some((gap, lines));
        }
    }
    best.map(|(_, l)| l).unwrap_or_default()
}

/// `StringSplitter.splitLines`: words onto lines no wider than `max`
/// (a word wider than a line is broken), `\n` breaking always.
fn split_lines(text: &str, max: f32, width: &impl Fn(&str) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate) <= max || line.is_empty() && width(word) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            let mut rest = word;
            while width(rest) > max {
                let cut = cut_to_width(rest, max, width);
                if cut.is_empty() {
                    break;
                }
                out.push(cut.clone());
                rest = &rest[cut.len()..];
            }
            line = rest.to_string();
        }
        out.push(line);
    }
    out
}

/// The part of `r` inside `within`.
fn clip(r: Rect, within: Rect) -> Option<Rect> {
    let (x0, y0) = (r.x.max(within.x), r.y.max(within.y));
    let (x1, y1) = (
        (r.x + r.w).min(within.x + within.w),
        (r.y + r.h).min(within.y + within.h),
    );
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Each advancement's place: `(column, row)` — the game's `TreeNodePosition`
/// (Walker's tidy tree), children in the order given; each root's tree
/// under the one before.
pub fn layout(nodes: &[Node]) -> Vec<(String, (f32, f32))> {
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut tree = Tree::default();
    let mut out = Vec::new();
    let mut offset = 0.0;
    for root in nodes.iter().filter(|n| parent_in(n, &ids).is_none()) {
        let r = tree.build(&root.id, None, None, 1, &Source { nodes, ids: &ids });
        tree.first_walk(r);
        let top = tree.second_walk(r, 0.0, 0, tree.n[r].y);
        if top < 0.0 {
            tree.third_walk(r, -top);
        }
        let mut bottom: f32 = 0.0;
        tree.each(r, &mut |n| {
            out.push((n.id.clone(), (n.x as f32, n.y + offset)));
            bottom = bottom.max(n.y);
        });
        offset += bottom + 1.0;
    }
    // A cycle leaves some unplaced: a column of their own.
    for n in nodes {
        if !out.iter().any(|(id, _)| *id == n.id) {
            out.push((n.id.clone(), (0.0, offset)));
            offset += 1.0;
        }
    }
    out
}

/// `n`'s parent among `ids` (none: a root).
fn parent_in<'a>(n: &'a Node, ids: &[&str]) -> Option<&'a str> {
    n.parent
        .as_deref()
        .filter(|p| ids.contains(p) && *p != n.id)
}

/// The advancements a tree is built from, and their ids.
struct Source<'a> {
    nodes: &'a [Node],
    ids: &'a [&'a str],
}

#[derive(Default)]
struct Tree {
    n: Vec<TreeNode>,
}

struct TreeNode {
    id: String,
    parent: Option<usize>,
    previous: Option<usize>,
    child_index: usize,
    children: Vec<usize>,
    ancestor: usize,
    thread: Option<usize>,
    x: i32,
    y: f32,
    modifier: f32,
    change: f32,
    shift: f32,
}

impl Tree {
    fn build(
        &mut self,
        id: &str,
        parent: Option<usize>,
        previous: Option<usize>,
        child_index: usize,
        src: &Source,
    ) -> usize {
        let me = self.n.len();
        let x = parent.map_or(0, |p| self.n[p].x + 1);
        self.n.push(TreeNode {
            id: id.to_string(),
            parent,
            previous,
            child_index,
            children: Vec::new(),
            ancestor: me,
            thread: None,
            x,
            y: -1.0,
            modifier: 0.0,
            change: 0.0,
            shift: 0.0,
        });
        let mut prev = None;
        for child in src
            .nodes
            .iter()
            .filter(|c| parent_in(c, src.ids) == Some(id))
        {
            if self.n.iter().any(|t| t.id == child.id) {
                continue;
            }
            let index = self.n[me].children.len() + 1;
            let c = self.build(&child.id, Some(me), prev, index, src);
            self.n[me].children.push(c);
            prev = Some(c);
        }
        me
    }

    fn each(&self, at: usize, f: &mut impl FnMut(&TreeNode)) {
        f(&self.n[at]);
        for &c in &self.n[at].children.clone() {
            self.each(c, f);
        }
    }

    fn first_walk(&mut self, v: usize) {
        let children = self.n[v].children.clone();
        if children.is_empty() {
            self.n[v].y = self.n[v].previous.map_or(0.0, |p| self.n[p].y + 1.0);
            return;
        }
        let mut default = None;
        for &c in &children {
            self.first_walk(c);
            default = Some(self.apportion(c, default.unwrap_or(c)));
        }
        self.execute_shifts(v);
        let (Some(&first), Some(&last)) = (children.first(), children.last()) else {
            return;
        };
        let mid = (self.n[first].y + self.n[last].y) / 2.0;
        match self.n[v].previous {
            Some(p) => {
                self.n[v].y = self.n[p].y + 1.0;
                self.n[v].modifier = self.n[v].y - mid;
            }
            None => self.n[v].y = mid,
        }
    }

    fn second_walk(&mut self, v: usize, offset: f32, column: i32, mut top: f32) -> f32 {
        self.n[v].y += offset;
        self.n[v].x = column;
        top = top.min(self.n[v].y);
        let m = self.n[v].modifier;
        for c in self.n[v].children.clone() {
            top = self.second_walk(c, offset + m, column + 1, top);
        }
        top
    }

    fn third_walk(&mut self, v: usize, y: f32) {
        self.n[v].y += y;
        for c in self.n[v].children.clone() {
            self.third_walk(c, y);
        }
    }

    fn execute_shifts(&mut self, v: usize) {
        let (mut shift, mut change) = (0.0, 0.0);
        for &c in self.n[v].children.clone().iter().rev() {
            self.n[c].y += shift;
            self.n[c].modifier += shift;
            change += self.n[c].change;
            shift += self.n[c].shift + change;
        }
    }

    fn previous_or_thread(&self, v: usize) -> Option<usize> {
        self.n[v]
            .thread
            .or_else(|| self.n[v].children.first().copied())
    }

    fn next_or_thread(&self, v: usize) -> Option<usize> {
        self.n[v]
            .thread
            .or_else(|| self.n[v].children.last().copied())
    }

    fn apportion(&mut self, v: usize, mut default: usize) -> usize {
        let (Some(previous), Some(parent)) = (self.n[v].previous, self.n[v].parent) else {
            return default;
        };
        let (mut in_right, mut out_right, mut in_left, mut out_left) =
            (v, v, previous, self.n[parent].children[0]);
        let (mut s_in_right, mut s_out_right) = (self.n[v].modifier, self.n[v].modifier);
        let (mut s_in_left, mut s_out_left) = (self.n[in_left].modifier, self.n[out_left].modifier);
        while let (Some(nl), Some(pr)) = (
            self.next_or_thread(in_left),
            self.previous_or_thread(in_right),
        ) {
            in_left = nl;
            in_right = pr;
            // The outer contours are as deep as the inner ones.
            let (Some(ol), Some(or)) = (
                self.previous_or_thread(out_left),
                self.next_or_thread(out_right),
            ) else {
                break;
            };
            (out_left, out_right) = (ol, or);
            self.n[out_right].ancestor = v;
            let gap = self.n[in_left].y + s_in_left - (self.n[in_right].y + s_in_right) + 1.0;
            if gap > 0.0 {
                let a = self.ancestor_of(in_left, v, default);
                self.move_subtree(a, v, gap);
                s_in_right += gap;
                s_out_right += gap;
            }
            s_in_left += self.n[in_left].modifier;
            s_in_right += self.n[in_right].modifier;
            s_out_left += self.n[out_left].modifier;
            s_out_right += self.n[out_right].modifier;
        }
        if self.next_or_thread(in_left).is_some() && self.next_or_thread(out_right).is_none() {
            self.n[out_right].thread = self.next_or_thread(in_left);
            self.n[out_right].modifier += s_in_left - s_out_right;
        } else {
            if self.previous_or_thread(in_right).is_some()
                && self.previous_or_thread(out_left).is_none()
            {
                self.n[out_left].thread = self.previous_or_thread(in_right);
                self.n[out_left].modifier += s_in_right - s_out_left;
            }
            default = v;
        }
        default
    }

    fn ancestor_of(&self, of: usize, v: usize, other: usize) -> usize {
        let a = self.n[of].ancestor;
        let siblings = self.n[v].parent.map(|p| &self.n[p].children);
        if siblings.is_some_and(|s| s.contains(&a)) {
            a
        } else {
            other
        }
    }

    fn move_subtree(&mut self, from: usize, to: usize, shift: f32) {
        let subtrees = self.n[to].child_index as f32 - self.n[from].child_index as f32;
        if subtrees != 0.0 {
            self.n[to].change -= shift / subtrees;
            self.n[from].change += shift / subtrees;
        }
        self.n[to].shift += shift;
        self.n[to].y += shift;
        self.n[to].modifier += shift;
    }
}

#[cfg(test)]
mod tests;
