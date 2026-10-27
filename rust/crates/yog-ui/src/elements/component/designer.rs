//! The screen designer's protocol (see the [module docs](super)): one C-ABI
//! entry point, `yog_ui_designer`, taking a [`Request`] and answering a
//! [`Reply`] — binary Yog Data both — through a write callback.
//!
//! - [`Request::Describe`] → [`Answer::Described`]: the mod's components.
//! - [`Request::Render`] → [`Answer::Rendered`]: builds the component,
//!   sizes it `w × h` (≤ 0: its natural size, within `avail_w × avail_h`),
//!   draws it at the origin through the `YogGfxApi`, and answers its size.
//!
//! A failure (unknown component, panic in `build`) answers the error.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use yog_abi::{YogBytes, YogGfxApi};
use yog_data::Schema;
use yog_gfx::GfxContext;

use super::{COMMON_ATTRS, PropKind, Props, apply_style, components, find};
use crate::widget::{self, Widget};
use crate::{FlexDir, UiRoot, Units};

/// Version of this protocol; the designer host refuses others.
pub const ABI: u32 = 2;

/// `write(user_data, bytes)`: the host's sink for the reply.
pub type WriteFn = unsafe extern "C" fn(user_data: *mut c_void, reply: YogBytes);

/// What the designer host asks a mod.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(tag = "request")]
pub enum Request {
    Describe,
    Render(Render),
}

/// A component to draw, at the origin.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct Render {
    pub name: String,
    /// Its attributes, as a `.yui` writes them.
    #[data(default)]
    pub attrs: Vec<(String, String)>,
    /// Its size; ≤ 0: its natural size.
    pub w: f32,
    pub h: f32,
    /// The room it has.
    pub avail_w: f32,
    pub avail_h: f32,
}

/// What a mod answers.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(tag = "answer")]
pub enum Answer {
    Described { components: Vec<ComponentInfo> },
    Rendered { w: f32, h: f32 },
}

/// The answer, or what went wrong.
pub type Reply = Result<Answer, String>;

/// One component as the mod describes it.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct ComponentInfo {
    pub name: String,
    /// Toolbox tooltip and property grid header (may be `@res:Set.key`).
    #[data(default)]
    pub description: String,
    /// The toolbox label (empty: `name`; may be `@res:Set.key`).
    #[data(default)]
    pub display_name: String,
    /// The toolbox icon — a Yog image source or `@res:Icons.x` (empty: a glyph).
    #[data(default)]
    pub icon: String,
    /// The toolbox category (empty: "<mod> Components"; may be `@res:Set.key`).
    #[data(default)]
    pub category: String,
    /// The canvas size for editing it, and an instance's size when dropped (0: unset).
    #[data(default)]
    pub design_w: f32,
    #[data(default)]
    pub design_h: f32,
    /// Whether the toolbox offers it.
    #[data(default = "yes")]
    pub toolbox: bool,
    #[data(default)]
    pub props: Vec<PropInfo>,
    /// The `.yui` declaring it (the designer's own listing; never the mod's).
    #[data(default)]
    pub file: Option<String>,
}

/// One property of a component.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct PropInfo {
    pub name: String,
    /// `text`, `number`, `int`, `bool`, `color`, `choice`, `event`, `image`.
    pub kind: String,
    #[data(default)]
    pub default: String,
    #[data(default)]
    pub doc: String,
    /// The property grid category (empty: the component's own).
    #[data(default)]
    pub category: String,
    #[data(default)]
    pub options: Vec<String>,
}

fn yes() -> bool {
    true
}

/// `export_mod!`'s `yog_ui_designer`.
///
/// # Safety
/// `request` must point at its bytes for the call; `gfx` a valid table (or
/// null for `describe`); `write` sound to call with `user_data`.
pub unsafe fn call(
    request: YogBytes,
    gfx: *const YogGfxApi,
    user_data: *mut c_void,
    write: WriteFn,
) -> bool {
    // SAFETY: the caller's contract.
    let request = unsafe { request.as_slice() };
    let reply: Reply = match catch_unwind(AssertUnwindSafe(|| answer(request, gfx))) {
        Ok(reply) => reply,
        Err(panic) => Err(panic_message(&*panic)),
    };
    let bytes = yog_data::to_bytes(&reply);
    // SAFETY: the caller's contract.
    unsafe { write(user_data, YogBytes::new(&bytes)) };
    reply.is_ok()
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    let msg = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into());
    format!("the component panicked: {msg}")
}

fn answer(request: &[u8], gfx: *const YogGfxApi) -> Reply {
    let request = yog_data::read_bytes::<Request>(request)
        .map_err(|e| format!("not a designer request: {e:?}"))?;
    match request {
        Request::Describe => Ok(describe()),
        Request::Render(r) => {
            if gfx.is_null() {
                return Err("render: no gfx table".into());
            }
            // SAFETY: non-null, valid for the call (the caller's contract).
            let ctx = unsafe { GfxContext::from_raw(gfx) };
            render(&ctx, &r)
        }
    }
}

fn render(ctx: &GfxContext, r: &Render) -> Reply {
    let spec = find(&r.name).ok_or_else(|| format!("no component named {}", r.name))?;
    let (w, h, aw, ah) = (r.w, r.h, r.avail_w, r.avail_h);
    type Attrs<'a> = Vec<(&'a str, &'a str)>;
    let (common, own): (Attrs, Attrs) = r
        .attrs
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
    Ok(Answer::Rendered {
        w: rect.w,
        h: rect.h,
    })
}

fn describe() -> Answer {
    let components = components()
        .iter()
        .map(|c| ComponentInfo {
            name: c.name.into(),
            description: c.doc.into(),
            display_name: c.display_name.into(),
            icon: c.icon.into(),
            category: c.category.into(),
            design_w: c.design_size.0,
            design_h: c.design_size.1,
            toolbox: c.toolbox,
            props: c
                .props
                .iter()
                .map(|p| PropInfo {
                    name: p.name.into(),
                    kind: p.kind.as_str().into(),
                    default: p.default.into(),
                    doc: p.doc.into(),
                    category: p.category.into(),
                    options: match p.kind {
                        PropKind::Choice(o) => o.iter().map(|s| s.to_string()).collect(),
                        _ => Vec::new(),
                    },
                })
                .collect(),
            file: None,
        })
        .collect();
    Answer::Described { components }
}
