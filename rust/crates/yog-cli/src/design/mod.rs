//! The `.yui` screen designer's backend: the canvas drawn by yog-ui itself
//! (the crate mods render with in game) through a recording `YogGfxApi`,
//! rasterized with Minecraft's own font and textures from the local game
//! files; a preview mode driving the same tree with real UI events
//! (`click:X:Y`, `scroll:DY`, scrollbar drags); and code-behind handlers.

pub mod advancements;
pub mod content;
pub mod data;
pub mod files;
pub mod mod_ui;
pub mod project;
pub mod recipe_kinds;
pub mod refs;
pub mod service;
pub mod worldgen;
pub mod yui_codegen;

/// This CLI as a command, for a design call that runs one of its
/// subcommands: the running binary when it's `yog`, else the `yog` on `PATH`.
pub fn this_cli() -> std::process::Command {
    let exe = std::env::current_exe().ok().filter(|e| e.file_stem().is_some_and(|s| s == yog_ids::program::CLI));
    std::process::Command::new(exe.unwrap_or_else(|| yog_ids::program::CLI.into()))
}
pub mod assets;
pub mod blocks3d;
pub mod item_models;
pub mod book;
pub mod codebehind;
pub mod components;
pub mod custom;
pub mod font;
pub mod font_copy;
mod font_yog;
pub mod gfx;
pub mod host;
pub mod images;
pub mod markup;
pub mod raster;
pub mod tags;
pub mod recipe_area;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use yog_sync::{Mutex, OnceLock};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use yog_gfx::Vec2;
use yog_ui::layout::LayoutNode;
use yog_ui::{ScrollState, UiRoot, Widget};

use self::assets::Resources;
use self::markup::YuiNode;

/// A laid-out widget: its border box and its content box (inside padding), GUI px.
#[derive(Debug, Clone, Serialize)]
pub struct NodeRect {
    pub uid: String,
    pub rect: [f32; 4],
    pub content: [f32; 4],
    /// A grid's rows and columns, `[start, size]` in GUI px (for its adorners).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<GridRects>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GridRects {
    pub rows: Vec<[f32; 2]>,
    pub cols: Vec<[f32; 2]>,
}

/// Where the game resources came from (`None`: no client jar found).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsInfo {
    pub version: String,
    pub jar: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    /// `data:image/png;base64,…` — `scale` physical px per GUI px.
    pub image: String,
    pub scale: u32,
    pub rects: Vec<NodeRect>,
    pub assets: Option<AssetsInfo>,
    /// Text uses the game's font (else a stand-in glyph box).
    pub real_font: bool,
}

/// The mod a file belongs to: the folder with its `yog.toml` (else the
/// folder holding the `src/` it's in).
pub fn mod_root(file: &Path) -> Option<PathBuf> {
    project::mod_root(file).or_else(|| {
        file.ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == "src"))
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
    })
}

/// Built and laid out like the generated `root_with`.
type Laid = UiRoot;

fn lay_out(root: &YuiNode, screen_w: f32, screen_h: f32, scroll: &HashMap<String, f32>) -> Laid {
    let mut ui = UiRoot::new(
        "yog-idle:designer",
        markup::screen(root, screen_w, screen_h, scroll),
    );
    // Text widths and image sizes come from the "host" — the designer's own
    // font and image resolution — as they do from the game in `UiRoot::render`.
    let table = gfx::api(screen_w as i32, screen_h as i32, 1.0);
    // SAFETY: every function in `table` is sound to call; only fn pointers are kept.
    let ctx = unsafe { yog_gfx::GfxContext::from_raw(&table) };
    yog_ui::text::use_host_metrics(&ctx);
    yog_ui::image::use_host_images(&ctx);
    ui.layout(screen_w, screen_h);
    ui
}

/// Pairs each `.yui` node with its widget and layout node (the screen's
/// wrapper skipped; a `TextBox`'s inner parts aren't elements).
fn walk<'a>(
    node: &'a YuiNode,
    w: &'a Widget,
    l: &'a LayoutNode,
    visit: &mut impl FnMut(&'a YuiNode, &'a Widget, &'a LayoutNode),
) {
    visit(node, w, l);
    if !markup::is_container(&node.kind) {
        return;
    }
    for (i, child) in markup::widget_children(node).enumerate() {
        if let (Some(cw), Some(cl)) = (w.children.get(i), l.children.get(i)) {
            walk(child, cw, cl, visit);
        }
    }
}

fn with_content<'a>(
    root: &'a YuiNode,
    laid: &'a Laid,
    mut visit: impl FnMut(&'a YuiNode, &'a Widget, &'a LayoutNode),
) {
    if let (Some(w), Some(l)) = (
        laid.root.children.first(),
        laid.layout_root.children.first(),
    ) {
        walk(root, w, l, &mut visit);
    }
}

fn rects(root: &YuiNode, laid: &Laid) -> Vec<NodeRect> {
    let mut out = Vec::new();
    with_content(root, laid, |n, w, l| {
        let r = l.rect;
        let p = w.style.pad;
        out.push(NodeRect {
            uid: n.uid.clone(),
            rect: [r.x, r.y, r.w, r.h],
            content: [
                r.x + p[3],
                r.y + p[0],
                (r.w - p[1] - p[3]).max(0.0),
                (r.h - p[0] - p[2]).max(0.0),
            ],
            grid: l.grid.as_ref().map(|g| GridRects {
                rows: g.rows.iter().map(|&(a, b)| [a, b]).collect(),
                cols: g.cols.iter().map(|&(a, b)| [a, b]).collect(),
            }),
        });
    });
    out
}

/// Per-file preview state: the scroll views' offsets/drags.
#[derive(Default)]
struct Preview {
    scroll: HashMap<String, ScrollState>,
}

fn previews() -> &'static Mutex<HashMap<String, Preview>> {
    static P: OnceLock<Mutex<HashMap<String, Preview>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

fn offsets(path: &str, preview: bool) -> HashMap<String, f32> {
    if !preview {
        return HashMap::new();
    }
    let p = previews().lock();
    p.get(path)
        .map(|s| {
            s.scroll
                .iter()
                .map(|(k, v)| (k.clone(), v.offset))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderArgs {
    pub path: String,
    pub root: YuiNode,
    pub screen_w: u32,
    pub screen_h: u32,
    pub scale: u32,
    #[serde(default)]
    pub preview: bool,
}

pub fn render(args: &RenderArgs) -> Result<Rendered, String> {
    let (sw, sh) = (args.screen_w.clamp(1, 4096), args.screen_h.clamp(1, 4096));
    let scroll = offsets(&args.path, args.preview);
    let root = mod_root(Path::new(&args.path));
    images::set_document(root.as_deref());
    // The mod's own components: expanded from markup, or marked spacers here drawn by the designer host below.
    let mut themed = args.root.clone();
    markup::apply_resources(&mut themed);
    markup::apply_theme(&mut themed);
    recipe_area::expand(&mut themed, root.as_deref());
    let prepared = custom::prepare(&themed, root.as_deref(), sw as f32, sh as f32);
    let laid = lay_out(&prepared.root, sw as f32, sh as f32, &scroll);
    let mut ops = gfx::record(sw as i32, sh as i32, |ctx| laid.render(ctx));
    let mut node_rects = rects(&prepared.root, &laid);
    if !prepared.is_empty() {
        custom::splice(
            &mut ops,
            &prepared,
            &node_rects.iter().map(|r| (r.uid.clone(), r.rect)).collect(),
        );
    }
    // The inside of an expanded component instance isn't the document's.
    node_rects.retain(|r| !r.uid.contains(components::INNER));
    let res = Resources::for_mod(root.as_deref());
    let scale = raster::fit_scale(sw, sh, args.scale);
    let canvas = raster::rasterize(&ops, sw, sh, scale, &res);
    let real_font = res.vanilla.is_some() || canvas_font_real(&res);
    let png = canvas.png()?;
    Ok(Rendered {
        image: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
        scale,
        rects: node_rects,
        assets: res.vanilla.as_ref().map(|v| AssetsInfo {
            version: v.version.clone(),
            jar: v.jar.to_string_lossy().into_owned(),
        }),
        real_font,
    })
}

fn canvas_font_real(res: &Resources) -> bool {
    font::Font::load(res, yog_ids::yog::DEFAULT_FONT).is_real()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventArgs {
    pub path: String,
    pub root: YuiNode,
    pub screen_w: u32,
    pub screen_h: u32,
    /// A Yog UI event: `click:X:Y`, `drag:X:Y`, `release:X:Y`, `scroll:DY`, `move:X:Y` (hover only).
    pub event: String,
    /// The cursor, for the wheel (which carries no position).
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResult {
    /// A scroll view used the event (wheel, scrollbar).
    pub scrolled: bool,
    /// The clickable element under the cursor (`hit_test`: deepest enabled with `on_click`).
    pub hit: Option<String>,
    /// Its `on_click`, for a click.
    pub clicked: Option<String>,
}

/// `hit_test` (yog-ui `layout::hit_test`), answering with the `.yui` uid.
fn hit<'a>(node: &'a YuiNode, l: &'a LayoutNode, mx: f32, my: f32) -> Option<&'a YuiNode> {
    if !l.rect.contains(Vec2::new(mx, my)) {
        return None;
    }
    if markup::is_container(&node.kind) {
        // Laid out from the widget children only — property elements have no layout node.
        let children: Vec<_> = markup::widget_children(node).zip(&l.children).collect();
        for (child, cl) in children.into_iter().rev() {
            if let Some(found) = hit(child, cl, mx, my) {
                return Some(found);
            }
        }
    }
    (l.on_click.is_some() && l.enabled).then_some(node)
}

fn xy(rest: &str) -> Option<(f32, f32)> {
    let mut it = rest.split(':');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

pub fn event(args: &EventArgs) -> EventResult {
    let (sw, sh) = (
        args.screen_w.clamp(1, 4096) as f32,
        args.screen_h.clamp(1, 4096) as f32,
    );
    let scroll = offsets(&args.path, true);
    let mod_root = mod_root(Path::new(&args.path));
    // Laid out like `render` does: images size by the same mod's resources.
    images::set_document(mod_root.as_deref());
    let mut themed = args.root.clone();
    markup::apply_resources(&mut themed);
    markup::apply_theme(&mut themed);
    recipe_area::expand(&mut themed, mod_root.as_deref());
    let prepared = custom::prepare(&themed, mod_root.as_deref(), sw, sh);
    let root = &prepared.root;
    let laid = lay_out(root, sw, sh, &scroll);
    let mut out = EventResult::default();
    let pos = args
        .event
        .split_once(':')
        .and_then(|(_, rest)| xy(rest))
        .unwrap_or((args.x, args.y));

    // Scroll views first, as a screen's code-behind does (`ScrollState::handle`).
    if !args.event.starts_with("move:") {
        let mut views: Vec<(&YuiNode, &LayoutNode)> = Vec::new();
        with_content(root, &laid, |n, _, l| {
            if n.kind == yog_ids::yui::SCROLL_VIEW {
                views.push((n, l));
            }
        });
        let mut all = previews().lock();
        let state = all.entry(args.path.clone()).or_default();
        let wheel = args.event.starts_with("scroll:");
        // The wheel goes to the innermost view under the cursor; the rest to every view (each checks its own scrollbar).
        for (n, l) in views.iter().rev() {
            if wheel && !l.rect.contains(Vec2::new(pos.0, pos.1)) {
                continue;
            }
            let s = state.scroll.entry(n.uid.clone()).or_default();
            if s.handle(&args.event, l, yog_ui::scroll::WHEEL_STEP) {
                out.scrolled = true;
                break;
            }
        }
    }
    if let Some(l) = laid.layout_root.children.first() {
        let found = hit(root, l, pos.0, pos.1);
        // Inside a component instance: the instance.
        out.hit = found.map(|n| {
            n.uid
                .split(components::INNER)
                .next()
                .unwrap_or_default()
                .to_string()
        });
        if args.event.starts_with("click:") && !out.scrolled {
            out.clicked = found.and_then(|n| n.attr(yog_ids::yui_attr::ON_CLICK)).map(str::to_string);
        }
    }
    out
}

/// Forgets a file's preview state (scroll positions).
pub fn reset(path: &str) {
    previews()
        .lock()
        .remove(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::raster::DrawOp;

    fn node(uid: &str, kind: &str, attrs: &[(&str, &str)], children: Vec<YuiNode>) -> YuiNode {
        YuiNode {
            uid: uid.into(),
            kind: kind.into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children,
        }
    }

    fn screen() -> YuiNode {
        node(
            "s",
            "Screen",
            &[
                ("id", "m:s"),
                ("width", "100"),
                ("height", "60"),
                ("bg", "#FF101010"),
            ],
            vec![
                node(
                    "b",
                    "Button",
                    &[
                        ("x", "10"),
                        ("y", "10"),
                        ("w", "40"),
                        ("h", "20"),
                        ("text", "Play"),
                        ("on_click", "play"),
                    ],
                    vec![],
                ),
                node(
                    "sv",
                    "ScrollView",
                    &[("x", "60"), ("y", "0"), ("w", "30"), ("h", "30")],
                    (0..10)
                        .map(|i| {
                            node(
                                &format!("r{i}"),
                                "Spacer",
                                &[("w", "20"), ("h", "10")],
                                vec![],
                            )
                        })
                        .collect(),
                ),
            ],
        )
    }

    #[test]
    fn rects_come_from_yog_ui_layout() {
        let laid = lay_out(&screen(), 200.0, 100.0, &HashMap::new());
        let r = rects(&screen(), &laid);
        // Centered: ((200 - 100) / 2, (100 - 60) / 2).
        assert_eq!(r[0].uid, "s");
        assert_eq!(r[0].rect, [50.0, 20.0, 100.0, 60.0]);
        let b = r.iter().find(|n| n.uid == "b").unwrap();
        assert_eq!(b.rect, [60.0, 30.0, 40.0, 20.0]);
    }

    #[test]
    fn grids_place_children_in_cells() {
        let defs = node(
            "d",
            "Grid.ColumnDefinitions",
            &[],
            vec![
                node("c0", "ColumnDefinition", &[("width", "30")], vec![]),
                node("c1", "ColumnDefinition", &[], vec![]),
            ],
        );
        let root = node(
            "s",
            "Screen",
            &[("width", "100"), ("height", "60")],
            vec![node(
                "g",
                "Grid",
                &[
                    ("w", "100"),
                    ("h", "60"),
                    ("rows", "Auto,*"),
                    ("column_spacing", "10"),
                ],
                vec![
                    defs,
                    node(
                        "a",
                        "Spacer",
                        &[("h", "12"), ("Grid.ColumnSpan", "2")],
                        vec![],
                    ),
                    node(
                        "b",
                        "Spacer",
                        &[("Grid.Row", "1"), ("Grid.Column", "1"), ("margin", "2")],
                        vec![],
                    ),
                ],
            )],
        );
        let laid = lay_out(&root, 100.0, 60.0, &HashMap::new());
        let r = rects(&root, &laid);
        let at = |uid: &str| r.iter().find(|n| n.uid == uid).unwrap();
        let g = at("g").grid.as_ref().unwrap();
        assert_eq!(g.rows, [[0.0, 12.0], [12.0, 48.0]]);
        assert_eq!(g.cols, [[0.0, 30.0], [40.0, 60.0]]);
        assert_eq!(at("a").rect, [0.0, 0.0, 100.0, 12.0]);
        assert_eq!(at("b").rect, [42.0, 14.0, 56.0, 44.0]);
    }

    #[test]
    fn renders_through_the_recorder() {
        let args = RenderArgs {
            path: "/nowhere/src/x.yui".into(),
            root: screen(),
            screen_w: 200,
            screen_h: 100,
            scale: 2,
            preview: false,
        };
        let laid = lay_out(&args.root, 200.0, 100.0, &HashMap::new());
        let ops = gfx::record(200, 100, |ctx| laid.render(ctx));
        assert!(
            matches!(
                ops.first(),
                Some(DrawOp::Rect {
                    color: 0xFF10_1010,
                    ..
                })
            ),
            "{ops:?}"
        );
        assert!(
            ops.iter()
                .any(|o| matches!(o, DrawOp::Text { text, .. } if text == "Play"))
        );
        let out = render(&args).unwrap();
        assert!(out.image.starts_with("data:image/png;base64,"));
        assert_eq!(out.rects.len(), 13);
    }

    #[test]
    fn preview_clicks_and_scrolls() {
        let path = "/nowhere/src/preview_test.yui";
        let ev = |e: &str, x: f32, y: f32| {
            event(&EventArgs {
                path: path.into(),
                root: screen(),
                screen_w: 200,
                screen_h: 100,
                event: e.into(),
                x,
                y,
            })
        };
        let r = ev("click:70:40", 0.0, 0.0);
        assert_eq!(r.clicked.as_deref(), Some("play"));
        assert_eq!(r.hit.as_deref(), Some("b"));
        assert!(ev("click:5:5", 0.0, 0.0).clicked.is_none());
        let r = ev("scroll:-1", 120.0, 30.0);
        assert!(r.scrolled);
        assert!(offsets(path, true)["sv"] > 0.0);
        reset(path);
        assert!(offsets(path, true).is_empty());
    }

    #[test]
    fn clicks_skip_property_elements() {
        let defs = node(
            "d",
            "Grid.ColumnDefinitions",
            &[],
            vec![node("c0", "ColumnDefinition", &[], vec![])],
        );
        let button = node(
            "b",
            "Button",
            &[("w", "40"), ("h", "20"), ("on_click", "go")],
            vec![],
        );
        let root = node(
            "s",
            "Screen",
            &[("width", "100"), ("height", "60")],
            vec![node(
                "g",
                "Grid",
                &[("w", "100"), ("h", "60")],
                vec![defs, button],
            )],
        );
        let r = event(&EventArgs {
            path: "/nowhere/src/grid_click.yui".into(),
            root,
            screen_w: 100,
            screen_h: 60,
            event: "click:50:30".into(),
            x: 0.0,
            y: 0.0,
        });
        assert_eq!(r.hit.as_deref(), Some("b"));
        assert_eq!(r.clicked.as_deref(), Some("go"));
    }
}

#[cfg(test)]
mod visual {
    use super::*;

    /// `YOG_DESIGNER_PNG=out.png cargo test --lib designer::visual -- --ignored`:
    /// a sample screen drawn with the local game files, for eyeballing.
    #[test]
    #[ignore]
    fn sample_png() {
        let Ok(out) = std::env::var(yog_ids::env::DESIGNER_PNG) else {
            return;
        };
        let n = |uid: &str, kind: &str, attrs: &[(&str, &str)]| YuiNode {
            uid: uid.into(),
            kind: kind.into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children: vec![],
        };
        let mut root = n(
            "s",
            "Screen",
            &[("width", "200"), ("height", "120"), ("bg", "#C0101010")],
        );
        root.children = vec![
            n(
                "l",
                "Label",
                &[
                    ("x", "6"),
                    ("y", "6"),
                    ("text", "Привет, Minecraft! §cRed §lBold"),
                    ("color", "#FFFFFF"),
                    ("no_wrap", "true"),
                ],
            ),
            n(
                "b",
                "Button",
                &[
                    ("x", "6"),
                    ("y", "24"),
                    ("w", "80"),
                    ("h", "20"),
                    ("text", "Play"),
                    ("bg", "#FF555555"),
                    ("color", "#FFFFFF"),
                ],
            ),
            n(
                "t",
                "TextBox",
                &[
                    ("x", "96"),
                    ("y", "26"),
                    ("w", "90"),
                    ("h", "16"),
                    ("placeholder", "Search…"),
                ],
            ),
            n(
                "i",
                "InvSlot",
                &[
                    ("x", "6"),
                    ("y", "50"),
                    ("item", "minecraft:diamond"),
                    ("count", "64"),
                ],
            ),
            n(
                "g",
                "Label",
                &[
                    ("x", "6"),
                    ("y", "76"),
                    ("text", "Galactic"),
                    ("font", "yog:font/alt"),
                    ("no_wrap", "true"),
                ],
            ),
            n(
                "u",
                "Label",
                &[
                    ("x", "6"),
                    ("y", "90"),
                    ("text", "日本語 ✓ ♥"),
                    ("no_wrap", "true"),
                ],
            ),
        ];
        let args = RenderArgs {
            path: "/nowhere/src/x.yui".into(),
            root,
            screen_w: 220,
            screen_h: 130,
            scale: 4,
            preview: false,
        };
        let r = render(&args).unwrap();
        let b64 = r.image.trim_start_matches("data:image/png;base64,");
        std::fs::write(
            out,
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap(),
        )
        .unwrap();
        eprintln!("assets: {:?}, real font: {}", r.assets, r.real_font);
    }
}
