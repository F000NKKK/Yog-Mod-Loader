//! Fitting the mod's components into the designer's picture: each laid out
//! in its slot, drawn by the mod's library, spliced in where it goes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use yog_sync::Arc;

use crate::design::drawing::raster::DrawOp;
use crate::design::host::RenderReq;
use crate::design::screens::components::{self, Def};
use crate::design::screens::custom::placeholder::{
    BUILDING, PLACEHOLDER_H, PLACEHOLDER_W, PROBLEM_H, placeholder,
};
use crate::design::screens::custom::{LAYOUT_ATTRS, Snapshot, Status, is_component, state, status};
use crate::design::screens::markup::YuiNode;

/// A custom element of the document being drawn.
pub(super) struct Slot {
    uid: String,
    marker: u32,
    pub(super) req: RenderReq,
    /// Why it can't be drawn (placeholder text).
    problem: Option<String>,
}

/// The document with custom elements replaced by marked spacers.
pub(crate) struct Prepared {
    pub(crate) root: YuiNode,
    pub(super) slots: Vec<Slot>,
    mod_root: Option<PathBuf>,
}

impl Prepared {
    pub(crate) fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// A component's stand-in color: alpha 1 (never a real color in a .yui,
/// invisible if left behind), a tag, and the component's index.
const MARKER: u32 = 0x01D0_0000;
const MARKER_INDEX: u32 = 0xFFFF;

pub(super) fn marker(i: usize) -> u32 {
    MARKER | (i as u32 & MARKER_INDEX)
}

fn px(v: Option<&str>) -> Option<f32> {
    let v = v?.trim();
    if v.ends_with('%') {
        None
    } else {
        v.parse().ok()
    }
}

/// The document as yog-ui should draw it: a `<Component>` document as a
/// screen of its design size; markup-only component instances expanded;
/// each compiled one (or unknown) measured through the host (its natural
/// size, when the element doesn't size it in px) and swapped for a marked
/// spacer.
pub(crate) fn prepare(
    root: &YuiNode,
    mod_root: Option<&Path>,
    screen_w: f32,
    screen_h: f32,
) -> Prepared {
    let root = if root.kind == yog_ids::yui::COMPONENT {
        components::design_root(root)
    } else {
        root.clone()
    };
    let mut prepared = Prepared {
        root,
        slots: Vec::new(),
        mod_root: mod_root.map(Path::to_path_buf),
    };
    if !has_custom(&prepared.root) {
        return prepared;
    }
    let mut cx = Cx {
        mod_root,
        defs: mod_root.map(components::scan).unwrap_or_default(),
        snapshot: None,
        slots: Vec::new(),
        screen: (screen_w, screen_h),
    };
    cx.process(&mut prepared.root, 0);
    prepared.slots = cx.slots;
    prepared
}

/// How deep components may nest (a component using itself stops here).
const MAX_NESTING: usize = 16;

struct Cx<'a> {
    mod_root: Option<&'a Path>,
    defs: Arc<Vec<Def>>,
    snapshot: Option<Snapshot>,
    slots: Vec<Slot>,
    screen: (f32, f32),
}

impl Cx<'_> {
    fn process(&mut self, node: &mut YuiNode, depth: usize) {
        if !is_component(&node.kind) {
            for child in &mut node.children {
                self.process(child, depth);
            }
            return;
        }
        let defs = self.defs.clone();
        match Def::find(&defs, &node.kind) {
            Some(def) if !def.compiled && depth < MAX_NESTING => {
                *node = components::expand(def, node);
                // The body's root may itself be a component.
                self.process(node, depth + 1);
            }
            def => {
                let problem = match def {
                    None => Some(format!(
                        "no component <{}> in this mod — add one: New Item → Component (.yui)",
                        node.kind
                    )),
                    Some(d) if !d.compiled => Some("this component contains itself".to_string()),
                    Some(_) => self.library_problem(&node.kind),
                };
                self.slot(node, problem);
            }
        }
    }

    /// Why a compiled component can't be drawn right now, if it can't.
    fn library_problem(&mut self, name: &str) -> Option<String> {
        let Some(mod_root) = self.mod_root else {
            return Some("not inside a mod (no src/ folder)".into());
        };
        let s = self.snapshot.get_or_insert_with(|| status(mod_root));
        match s.status {
            Status::Building | Status::None => Some(format!("{BUILDING} designer library…")),
            Status::Failed => Some(format!(
                "The designer could not load the mod's library:\n{}",
                s.error.clone().unwrap_or_default()
            )),
            Status::Ready => {
                let registered = state(mod_root)
                    .lock()
                    .components
                    .iter()
                    .any(|c| c.name == name);
                (!registered).then(|| format!("the mod's library doesn't register <{name}> — does its code-behind call yog_api::ui_include!?"))
            }
        }
    }

    fn slot(&mut self, node: &mut YuiNode, problem: Option<String>) {
        let name = node.kind.clone();
        let props: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| !LAYOUT_ATTRS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let (w, h) = (
            px(node.attr(yog_ids::yui_attr::W)),
            px(node.attr(yog_ids::yui_attr::H)),
        );
        let req = RenderReq {
            name,
            attrs: props,
            w: w.unwrap_or(0.0),
            h: h.unwrap_or(0.0),
            avail_w: self.screen.0,
            avail_h: self.screen.1,
        };
        let (mut nw, mut nh) = (
            w.unwrap_or(PLACEHOLDER_W),
            h.unwrap_or(if problem.is_some() {
                PROBLEM_H
            } else {
                PLACEHOLDER_H
            }),
        );
        let problem = match (problem, self.mod_root) {
            (None, Some(root)) if w.is_none() || h.is_none() => {
                match state(root).lock().render(&req) {
                    Ok((mw, mh, _)) => {
                        nw = w.unwrap_or(mw);
                        nh = h.unwrap_or(mh);
                        None
                    }
                    Err(e) => Some(e),
                }
            }
            (p, _) => p,
        };
        let i = self.slots.len();
        let mut attrs: HashMap<String, String> = node
            .attrs
            .iter()
            .filter(|(k, _)| LAYOUT_ATTRS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        attrs
            .entry(yog_ids::yui_attr::W.into())
            .or_insert_with(|| nw.to_string());
        attrs
            .entry(yog_ids::yui_attr::H.into())
            .or_insert_with(|| nh.to_string());
        attrs.insert(yog_ids::yui_attr::BG.into(), format!("#{:08X}", marker(i)));
        self.slots.push(Slot {
            uid: node.uid.clone(),
            marker: marker(i),
            req,
            problem,
        });
        *node = YuiNode {
            uid: node.uid.clone(),
            kind: yog_ids::yui::SPACER.into(),
            attrs,
            children: Vec::new(),
        };
    }
}

fn has_custom(node: &YuiNode) -> bool {
    is_component(&node.kind) || node.children.iter().any(has_custom)
}

/// Replaces each marker fill in `ops` with the component drawn at its
/// element's laid-out rect (`rects`: uid → `x y w h`), clipped to the part
/// that was visible (the marker fill, already cut to scroll views).
pub(crate) fn splice(
    ops: &mut Vec<DrawOp>,
    prepared: &Prepared,
    rects: &HashMap<String, [f32; 4]>,
) {
    if prepared.slots.is_empty() {
        return;
    }
    let shared = prepared.mod_root.as_deref().map(state);
    let mut out = Vec::with_capacity(ops.len());
    for op in ops.drain(..) {
        let DrawOp::Rect {
            x1,
            y1,
            x2,
            y2,
            color,
        } = op
        else {
            out.push(op);
            continue;
        };
        let Some(slot) = prepared.slots.iter().find(|s| s.marker == color) else {
            out.push(op);
            continue;
        };
        let [x, y, w, h] = rects
            .get(&slot.uid)
            .copied()
            .unwrap_or([x1, y1, x2 - x1, y2 - y1]);
        let drawn = match (&slot.problem, &shared) {
            (Some(p), _) => Err(p.clone()),
            (None, None) => Err("not inside a mod".into()),
            (None, Some(sh)) => sh
                .lock()
                .render(&RenderReq {
                    w,
                    h,
                    ..slot.req.clone()
                })
                .map(|(_, _, ops)| ops),
        };
        let inner = drawn.unwrap_or_else(|e| placeholder(&slot.req.name, &e, w, h));
        out.push(DrawOp::Scissor {
            x: x1.floor() as i32,
            y: y1.floor() as i32,
            w: (x2.ceil() - x1.floor()) as i32,
            h: (y2.ceil() - y1.floor()) as i32,
        });
        out.extend(inner.into_iter().map(|o| o.moved(x, y)));
        out.push(DrawOp::ClearScissor);
    }
    *ops = out;
}
