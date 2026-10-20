//! The screen designer's protocol (see the [module docs](super)): one C-ABI
//! entry point, `yog_ui_designer`, taking a text request and answering
//! through a write callback.
//!
//! Requests (lines; values escaped with `\\` and `\n`):
//!
//! - `describe` → JSON `{"abi":1,"components":[{"name","doc","props":[{"name","kind","default","doc","options"}]}]}`
//! - `render` / `<name>` / `<w> <h> <avail_w> <avail_h>` / `key=value`… —
//!   builds the component, sizes it `w × h` (≤ 0: its natural size, within
//!   `avail_w × avail_h`), draws it at the origin through the `YogGfxApi`
//!   and answers `{"w":…,"h":…}`.
//!
//! A failure (unknown component, panic in `build`) answers `false` and an
//! error message.

use std::ffi::c_void;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};

use yog_abi::{YogGfxApi, YogStr};
use yog_gfx::GfxContext;

use super::{COMMON_ATTRS, PropKind, Props, apply_style, components, find};
use crate::widget::{self, Widget};
use crate::{FlexDir, UiRoot, Units};

/// Version of this protocol; the designer host refuses others.
pub const ABI: u32 = 1;

/// `write(user_data, text)`: the host's sink for the answer.
pub type WriteFn = unsafe extern "C" fn(user_data: *mut c_void, text: YogStr);

/// `export_mod!`'s `yog_ui_designer`.
///
/// # Safety
/// `request` must be valid UTF-8 for the call; `gfx` a valid table (or
/// null for `describe`); `write` sound to call with `user_data`.
pub unsafe fn call(
    request: YogStr,
    gfx: *const YogGfxApi,
    user_data: *mut c_void,
    write: WriteFn,
) -> bool {
    // SAFETY: the caller's contract.
    let request = unsafe { request.as_str() };
    let (ok, answer) = match catch_unwind(AssertUnwindSafe(|| answer(request, gfx))) {
        Ok(Ok(a)) => (true, a),
        Ok(Err(e)) => (false, e),
        Err(panic) => (false, panic_message(&*panic)),
    };
    // SAFETY: the caller's contract.
    unsafe { write(user_data, YogStr::from_str(&answer)) };
    ok
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    let msg = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into());
    format!("the component panicked: {msg}")
}

fn answer(request: &str, gfx: *const YogGfxApi) -> Result<String, String> {
    let mut lines = request.lines();
    match lines.next().map(str::trim) {
        Some("describe") => Ok(describe()),
        Some("render") => {
            if gfx.is_null() {
                return Err("render: no gfx table".into());
            }
            let name = lines.next().unwrap_or_default().trim().to_string();
            let size: Vec<f32> = lines
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            let [w, h, aw, ah] = size[..] else {
                return Err("render: expected `w h avail_w avail_h`".into());
            };
            let attrs: Vec<(String, String)> = lines
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.trim().to_string(), unescape(v)))
                .collect();
            // SAFETY: non-null, valid for the call (the caller's contract).
            let ctx = unsafe { GfxContext::from_raw(gfx) };
            render(&ctx, &name, &attrs, [w, h, aw, ah])
        }
        other => Err(format!("unknown designer request {other:?}")),
    }
}

fn render(
    ctx: &GfxContext,
    name: &str,
    attrs: &[(String, String)],
    [w, h, aw, ah]: [f32; 4],
) -> Result<String, String> {
    let spec = find(name).ok_or_else(|| format!("no component named {name}"))?;
    type Attrs<'a> = Vec<(&'a str, &'a str)>;
    let (common, own): (Attrs, Attrs) = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .partition(|(k, _)| COMMON_ATTRS.contains(k));
    let mut root: Widget = (spec.build)(&Props::new(spec.props, &own));
    for (k, v) in common {
        root = apply_style(root, k, v);
    }
    if w > 0.0 {
        root = root.w(Units::Px(w));
    }
    if h > 0.0 {
        root = root.h(Units::Px(h));
    }
    let (aw, ah) = (aw.max(1.0), ah.max(1.0));
    let screen = widget::panel(FlexDir::Column)
        .w(Units::Px(aw))
        .h(Units::Px(ah))
        .child(root.at(0.0, 0.0));
    // Measured like the game measures: through the host's text/image metrics.
    crate::text::use_host_metrics(ctx);
    crate::image::use_host_images(ctx);
    let mut ui = UiRoot::new(yog_ids::yog::DESIGNER_SCREEN, screen);
    ui.layout(aw, ah);
    let rect = ui
        .layout_root
        .children
        .first()
        .map(|n| n.rect)
        .unwrap_or_default();
    ui.render(ctx);
    Ok(format!("{{\"w\":{},\"h\":{}}}", rect.w, rect.h))
}

fn describe() -> String {
    let mut out = format!("{{\"abi\":{ABI},\"components\":[");
    for (i, c) in components().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"name\":{},\"description\":{},\"display_name\":{},\"icon\":{},\"category\":{},\"design_w\":{},\"design_h\":{},\"toolbox\":{},\"props\":[",
            json(c.name),
            json(c.doc),
            json(c.display_name),
            json(c.icon),
            json(c.category),
            c.design_size.0,
            c.design_size.1,
            c.toolbox
        );
        for (j, p) in c.props.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let options = match p.kind {
                PropKind::Choice(o) => o.iter().map(|s| json(s)).collect::<Vec<_>>().join(","),
                _ => String::new(),
            };
            let _ = write!(
                out,
                "{{\"name\":{},\"kind\":{},\"default\":{},\"doc\":{},\"category\":{},\"options\":[{options}]}}",
                json(p.name),
                json(p.kind.as_str()),
                json(p.default),
                json(p.doc),
                json(p.category)
            );
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// A JSON string literal.
fn json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A request value as written by [`escape`].
pub fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// A request value: one line, `\\` and `\n` escaped.
pub fn escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n")
}
