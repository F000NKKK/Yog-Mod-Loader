//! The designer host: a child process of the IDE (`yog designer-host`)
//! that loads a mod's library built for this machine (`yog designer-lib`)
//! and draws the mod's custom `.yui` components through the same recording
//! `YogGfxApi` the designer uses — the mod's own yog-ui and `build` code,
//! out of the IDE's process, so a panicking/crashing/looping component (or
//! a rebuilt library) costs a restart of the host, never the IDE.
//!
//! Protocol: one JSON request per stdin line; one JSON answer per stdout
//! line, prefixed with [`MARK`] (the mod's code may print to stdout too).
//!
//! - `{"cmd":"load","lib":…,"modRoot":…}` → `{"ok":true,"components":[…]}`
//! - `{"cmd":"render","name":…,"props":[[k,v]…],"w":…,"h":…,"availW":…,"availH":…}`
//!   → `{"ok":true,"w":…,"h":…,"ops":[DrawOp…]}` (drawn at the origin; `w`/`h` ≤ 0: natural size)
//! - `{"cmd":"png", …render…, "scale":4, "out":"x.png"}` → the same, also
//!   rasterized into `out` (for checking a component from a shell)
//!
//! Failures answer `{"ok":false,"error":…}`.

use std::ffi::c_void;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use yog_abi::{YogGfxApi, YogStr};

use super::raster::DrawOp;
use super::{gfx, images, raster};

/// Starts every answer line on stdout.
pub const MARK: &str = "\u{1}yog-designer\u{1}";

/// The designer protocol version this host speaks (`yog_ui::component::designer::ABI`).
pub const DESIGNER_ABI: u32 = 1;

type DesignerFn = unsafe extern "C" fn(
    YogStr,
    *const YogGfxApi,
    *mut c_void,
    unsafe extern "C" fn(*mut c_void, YogStr),
) -> bool;

/// A loaded mod library.
pub struct Loaded {
    _lib: libloading::Library,
    call: DesignerFn,
}

/// One component as the mod describes it (`describe`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentInfo {
    pub name: String,
    /// Toolbox tooltip and property grid header (may be `@Set.key`).
    #[serde(default)]
    pub description: String,
    /// The toolbox label (empty: `name`; may be `@Set.key`).
    #[serde(default)]
    pub display_name: String,
    /// The toolbox icon — a Yog image source or `@Icons.x` (empty: a glyph).
    #[serde(default)]
    pub icon: String,
    /// The toolbox category (empty: "<mod> Components"; may be `@Set.key`).
    #[serde(default)]
    pub category: String,
    /// The canvas size for editing it, and an instance's size when dropped (0: unset).
    #[serde(default)]
    pub design_w: f32,
    #[serde(default)]
    pub design_h: f32,
    /// Whether the toolbox offers it.
    #[serde(default = "yes")]
    pub toolbox: bool,
    #[serde(default)]
    pub props: Vec<PropInfo>,
    /// The `.yui` declaring it (the designer's own listing; not from the host).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropInfo {
    pub name: String,
    /// `text`, `number`, `int`, `bool`, `color`, `choice`, `event`, `image`.
    pub kind: String,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub doc: String,
    /// The property grid category (empty: the component's own).
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub options: Vec<String>,
}

fn yes() -> bool {
    true
}

unsafe extern "C" fn sink(user_data: *mut c_void, text: YogStr) {
    // SAFETY: `user_data` is the `String` `Loaded::call` passes; `text` is valid for the call.
    unsafe { (*(user_data as *mut String)).push_str(text.as_str()) };
}

impl Loaded {
    /// Loads `lib`, checking it speaks this IDE's gfx ABI and designer protocol.
    pub fn open(lib: &Path) -> Result<Self, String> {
        // SAFETY: loading runs the library's constructors — the mod's own
        // code, which is exactly what this (disposable) process is for.
        let lib = unsafe { libloading::Library::new(lib) }
            .map_err(|e| format!("can't load {}: {e}", lib.display()))?;
        // SAFETY: symbols of `export_mod!` with these signatures.
        unsafe {
            let abi = lib
                .get::<unsafe extern "C" fn() -> u32>(b"yog_abi_version\0")
                .map_err(|_| {
                    "not a Yog mod library (no yog_abi_version — is `export_mod!` missing?)"
                        .to_string()
                })?;
            let abi = abi();
            if abi != yog_abi::ABI_VERSION {
                return Err(format!(
                    "the mod is built against Yog ABI {abi}, the designer against {} — update yog-api (or Yog-IDLE) so they match",
                    yog_abi::ABI_VERSION
                ));
            }
            let designer = lib
                .get::<unsafe extern "C" fn() -> u32>(b"yog_ui_designer_abi\0")
                .map_err(|_| {
                    "the mod's yog-api has no designer support (yog_ui_designer) — update yog-api"
                        .to_string()
                })?;
            let designer = designer();
            if designer != DESIGNER_ABI {
                return Err(format!(
                    "designer protocol {designer} (this IDE speaks {DESIGNER_ABI}) — update yog-api or Yog-IDLE"
                ));
            }
            let call = *lib
                .get::<DesignerFn>(b"yog_ui_designer\0")
                .map_err(|e| e.to_string())?;
            Ok(Self { _lib: lib, call })
        }
    }

    fn call(&self, request: &str, gfx: *const YogGfxApi) -> Result<String, String> {
        let mut out = String::new();
        // SAFETY: `request` lives for the call; `gfx` is null or a valid table; `sink` matches `out`.
        let ok = unsafe {
            (self.call)(
                YogStr::from_str(request),
                gfx,
                &mut out as *mut String as *mut c_void,
                sink,
            )
        };
        if ok { Ok(out) } else { Err(out) }
    }

    pub fn describe(&self) -> Result<Vec<ComponentInfo>, String> {
        let answer = self.call("describe", std::ptr::null())?;
        #[derive(Deserialize)]
        struct Described {
            components: Vec<ComponentInfo>,
        }
        serde_json::from_str::<Described>(&answer)
            .map(|d| d.components)
            .map_err(|e| format!("bad describe answer: {e}"))
    }

    /// Draws `name` at the origin; returns its size and what it drew.
    pub fn render(&self, req: &RenderReq) -> Result<(f32, f32, Vec<DrawOp>), String> {
        let mut request = format!(
            "render\n{}\n{} {} {} {}\n",
            req.name, req.w, req.h, req.avail_w, req.avail_h
        );
        for (k, v) in &req.props {
            request.push_str(k);
            request.push('=');
            request.push_str(&v.replace('\\', "\\\\").replace('\n', "\\n"));
            request.push('\n');
        }
        let (aw, ah) = (req.avail_w.max(1.0) as i32, req.avail_h.max(1.0) as i32);
        let mut answer = Err(String::new());
        // The same recording functions as `record`'s own table, so they record into its list.
        let table = gfx::api(aw, ah, 1.0);
        let ops = gfx::record(aw, ah, |_| answer = self.call(&request, &table));
        let answer = answer?;
        let size: Value =
            serde_json::from_str(&answer).map_err(|e| format!("bad render answer: {e}"))?;
        let dim = |k: &str| size.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        Ok((dim("w"), dim("h"), ops))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderReq {
    pub name: String,
    #[serde(default)]
    pub props: Vec<(String, String)>,
    #[serde(default)]
    pub w: f32,
    #[serde(default)]
    pub h: f32,
    #[serde(default = "big")]
    pub avail_w: f32,
    #[serde(default = "big")]
    pub avail_h: f32,
}

fn big() -> f32 {
    1024.0
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
enum Request {
    #[serde(rename_all = "camelCase")]
    Load {
        lib: PathBuf,
        mod_root: Option<PathBuf>,
    },
    Render(RenderReq),
    Png {
        #[serde(flatten)]
        req: RenderReq,
        #[serde(default)]
        scale: Option<u32>,
        out: PathBuf,
    },
}

struct Host {
    lib: Option<Loaded>,
    mod_root: Option<PathBuf>,
}

impl Host {
    fn handle(&mut self, line: &str) -> Value {
        let request: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => return json!({"ok": false, "error": format!("bad request: {e}")}),
        };
        let result = match request {
            Request::Load { lib, mod_root } => {
                if self.lib.is_some() {
                    return json!({"ok": false, "error": "a library is already loaded (restart the host to reload)"});
                }
                images::set_document(mod_root.as_deref());
                self.mod_root = mod_root;
                Loaded::open(&lib).and_then(|l| {
                    let components = l.describe()?;
                    self.lib = Some(l);
                    Ok(json!({"ok": true, "components": components}))
                })
            }
            Request::Render(req) => self
                .loaded()
                .and_then(|l| l.render(&req))
                .map(|(w, h, ops)| json!({"ok": true, "w": w, "h": h, "ops": ops})),
            Request::Png { req, scale, out } => self
                .loaded()
                .and_then(|l| l.render(&req))
                .and_then(|(w, h, ops)| {
                    let res = super::assets::Resources::for_mod(self.mod_root.as_deref());
                    let (cw, ch) = (w.ceil().max(1.0) as u32, h.ceil().max(1.0) as u32);
                    let canvas =
                        raster::rasterize(&ops, cw, ch, scale.unwrap_or(4).clamp(1, 8), &res);
                    std::fs::write(&out, canvas.png()?)
                        .map_err(|e| format!("{}: {e}", out.display()))?;
                    Ok(json!({"ok": true, "w": w, "h": h, "ops": ops.len(), "out": out}))
                }),
        };
        result.unwrap_or_else(|e| json!({"ok": false, "error": e}))
    }

    fn loaded(&self) -> Result<&Loaded, String> {
        self.lib
            .as_ref()
            .ok_or_else(|| "no library loaded".to_string())
    }
}

/// The CLI command that runs a host.
pub const COMMAND: &str = "designer-host";

/// `yog designer-host`: serves requests until stdin closes.
pub fn serve() {
    let mut host = Host {
        lib: None,
        mod_root: None,
    };
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let answer = host.handle(&line);
        let mut out = std::io::stdout().lock();
        if writeln!(out, "{MARK}{answer}")
            .and_then(|_| out.flush())
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_parse() {
        let r: Request = serde_json::from_str(
            r#"{"cmd":"render","name":"StatBar","props":[["value","0.5"]],"w":0}"#,
        )
        .unwrap();
        assert!(
            matches!(r, Request::Render(RenderReq { ref name, ref props, avail_w, .. }) if name == "StatBar" && props.len() == 1 && avail_w == 1024.0)
        );
        let mut host = Host {
            lib: None,
            mod_root: None,
        };
        assert_eq!(
            host.handle(r#"{"cmd":"render","name":"X"}"#)["error"],
            "no library loaded"
        );
        assert!(
            host.handle(r#"{"cmd":"load","lib":"/nowhere/libnope.so"}"#)["error"]
                .as_str()
                .unwrap()
                .contains("can't load")
        );
    }

    /// `YOG_DESIGNER_LIB=…/libmod.so YOG_DESIGNER_COMPONENT=StatBar [YOG_DESIGNER_PNG=out.png]
    /// cargo test --lib designer::host -- --ignored` (`YOG_DESIGNER_PROPS="k=v;k=v"`)
    #[test]
    #[ignore]
    fn renders_a_built_mod() {
        let (Ok(lib), Ok(name)) = (
            std::env::var("YOG_DESIGNER_LIB"),
            std::env::var("YOG_DESIGNER_COMPONENT"),
        ) else {
            return;
        };
        let props = std::env::var("YOG_DESIGNER_PROPS").unwrap_or_default();
        let props = props
            .split(';')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut host = Host {
            lib: None,
            mod_root: None,
        };
        let loaded = host.handle(&json!({"cmd": "load", "lib": lib}).to_string());
        eprintln!("{loaded:#}");
        let l = host.loaded().unwrap();
        let (w, h, ops) = l
            .render(&RenderReq {
                name: name.clone(),
                props,
                w: 0.0,
                h: 0.0,
                avail_w: 400.0,
                avail_h: 300.0,
            })
            .unwrap();
        eprintln!("{w}x{h}: {ops:?}");
        assert!(w > 0.0 && h > 0.0 && !ops.is_empty());
        if let Ok(out) = std::env::var("YOG_DESIGNER_PNG") {
            let res = super::super::assets::Resources::for_mod(None);
            let canvas = raster::rasterize(&ops, w.ceil() as u32, h.ceil() as u32, 6, &res);
            std::fs::write(&out, canvas.png().unwrap()).unwrap();
        }
    }
}
