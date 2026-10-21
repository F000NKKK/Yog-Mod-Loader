//! The designer host: a child process of the IDE (`yog designer-host`)
//! that loads a mod's library built for this machine (`yog designer-lib`)
//! and draws the mod's custom `.yui` components through the same recording
//! `YogGfxApi` the designer uses — the mod's own yog-ui and `build` code,
//! out of the IDE's process, so a panicking/crashing/looping component (or
//! a rebuilt library) costs a restart of the host, never the IDE.
//!
//! Protocol: a [`HostRequest`] per frame of binary Yog Data on stdin, a
//! [`HostReply`] per frame on stdout, each after [`MARK`] (the mod's code
//! may print to stdout too; [`next_answer`] skips what it prints).
//!
//! - `Load` → `Loaded` (the mod's components)
//! - `Render` → `Drawn` (its size and what it drew, at the origin; `w`/`h` ≤ 0: natural size)
//! - `Png` → `Written`: the same, rasterized into `out` (for checking a component from a shell)

use std::ffi::c_void;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use yog_abi::{YogBytes, YogGfxApi};
use yog_data::Schema;
use yog_ui::component::designer::{self as protocol, Answer};

pub use yog_ui::component::designer::{ComponentInfo, PropInfo, Render as RenderReq};

use super::raster::DrawOp;
use super::{gfx, images, raster};

/// Starts every answer on stdout.
pub const MARK: &[u8] = b"\x01yog-designer\x01";

/// The designer protocol version this host speaks.
pub const DESIGNER_ABI: u32 = protocol::ABI;

/// How finely `Png` rasterizes when not told, and the finest it may.
const PNG_SCALE: u32 = 4;
const MAX_PNG_SCALE: u32 = 8;

type DesignerFn =
    unsafe extern "C" fn(YogBytes, *const YogGfxApi, *mut c_void, protocol::WriteFn) -> bool;

/// A loaded mod library.
pub struct Loaded {
    _lib: libloading::Library,
    call: DesignerFn,
}

unsafe extern "C" fn sink(user_data: *mut c_void, reply: YogBytes) {
    // SAFETY: `user_data` is the `Vec` `Loaded::ask` passes; `reply` is valid for the call.
    unsafe { (*(user_data as *mut Vec<u8>)).extend_from_slice(reply.as_slice()) };
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

    /// The mod's answer to `request` (`gfx`: null, or the table to draw through).
    fn ask(&self, request: &protocol::Request, gfx: *const YogGfxApi) -> Result<Answer, String> {
        let request = yog_data::to_bytes(request);
        let mut reply = Vec::new();
        // SAFETY: `request` lives for the call; `gfx` is null or a valid table; `sink` matches `reply`.
        unsafe {
            (self.call)(
                YogBytes::new(&request),
                gfx,
                &mut reply as *mut Vec<u8> as *mut c_void,
                sink,
            )
        };
        yog_data::read_bytes::<protocol::Reply>(&reply)
            .map_err(|e| format!("a mod's answer that isn't one: {e:?}"))?
    }

    pub fn describe(&self) -> Result<Vec<ComponentInfo>, String> {
        match self.ask(&protocol::Request::Describe, std::ptr::null())? {
            Answer::Described { components } => Ok(components),
            other => Err(format!("asked to describe, the mod answered {other:?}")),
        }
    }

    /// Draws `req.name` at the origin; returns its size and what it drew.
    pub fn render(&self, req: &RenderReq) -> Result<(f32, f32, Vec<DrawOp>), String> {
        let (aw, ah) = (req.avail_w.max(1.0) as i32, req.avail_h.max(1.0) as i32);
        let mut answer = Err(String::new());
        // The same recording functions as `record`'s own table, so they record into its list.
        let table = gfx::api(aw, ah, 1.0);
        let request = protocol::Request::Render(req.clone());
        let ops = gfx::record(aw, ah, |_| answer = self.ask(&request, &table));
        match answer? {
            Answer::Rendered { w, h } => Ok((w, h, ops)),
            other => Err(format!("asked to render, the mod answered {other:?}")),
        }
    }
}

/// What the IDE's side asks the host.
#[derive(Debug, Clone, Schema)]
#[data(tag = "cmd")]
pub enum HostRequest {
    Load {
        lib: PathBuf,
        mod_root: Option<PathBuf>,
    },
    Render(RenderReq),
    Png {
        req: RenderReq,
        scale: Option<u32>,
        out: PathBuf,
    },
}

/// What the host answers.
#[derive(Debug, Clone, Schema)]
#[data(tag = "answer")]
pub enum HostAnswer {
    Loaded { components: Vec<ComponentInfo> },
    Drawn { w: f32, h: f32, ops: Vec<DrawOp> },
    Written { w: f32, h: f32, out: PathBuf },
}

/// The answer, or what went wrong.
pub type HostReply = Result<HostAnswer, String>;

/// The next answer on the host's stdout: what the mod printed skipped up to
/// [`MARK`], then the frame; `None` once the host has gone.
pub fn next_answer(out: &mut impl BufRead) -> io::Result<Option<HostReply>> {
    let mut seen = Vec::new();
    let last = MARK[MARK.len() - 1];
    loop {
        if out.read_until(last, &mut seen)? == 0 {
            return Ok(None);
        }
        if seen.ends_with(MARK) {
            return yog_data::io::read_frame(out);
        }
    }
}

struct Host {
    lib: Option<Loaded>,
    mod_root: Option<PathBuf>,
}

impl Host {
    fn handle(&mut self, request: HostRequest) -> HostReply {
        match request {
            HostRequest::Load { lib, mod_root } => {
                if self.lib.is_some() {
                    return Err("a library is already loaded (restart the host to reload)".into());
                }
                images::set_document(mod_root.as_deref());
                self.mod_root = mod_root;
                let loaded = Loaded::open(&lib)?;
                let components = loaded.describe()?;
                self.lib = Some(loaded);
                Ok(HostAnswer::Loaded { components })
            }
            HostRequest::Render(req) => self
                .loaded()?
                .render(&req)
                .map(|(w, h, ops)| HostAnswer::Drawn { w, h, ops }),
            HostRequest::Png { req, scale, out } => {
                let (w, h, ops) = self.loaded()?.render(&req)?;
                let res = super::assets::Resources::for_mod(self.mod_root.as_deref());
                let (cw, ch) = (w.ceil().max(1.0) as u32, h.ceil().max(1.0) as u32);
                let scale = scale.unwrap_or(PNG_SCALE).clamp(1, MAX_PNG_SCALE);
                let canvas = raster::rasterize(&ops, cw, ch, scale, &res);
                std::fs::write(&out, canvas.png()?)
                    .map_err(|e| format!("{}: {e}", out.display()))?;
                Ok(HostAnswer::Written { w, h, out })
            }
        }
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
    let mut input = io::stdin().lock();
    while let Ok(Some(bytes)) = yog_data::io::read_frame_bytes(&mut input) {
        let reply = match yog_data::read_bytes::<HostRequest>(&bytes) {
            Ok(request) => host.handle(request),
            Err(e) => Err(format!("bad request: {e:?}")),
        };
        let mut out = io::stdout().lock();
        let sent = out
            .write_all(MARK)
            .and_then(|()| yog_data::io::write_frame(&mut out, &reply));
        if sent.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_answered() {
        let mut host = Host {
            lib: None,
            mod_root: None,
        };
        let render = HostRequest::Render(RenderReq {
            name: "X".into(),
            attrs: Vec::new(),
            w: 0.0,
            h: 0.0,
            avail_w: 1.0,
            avail_h: 1.0,
        });
        assert_eq!(host.handle(render).unwrap_err(), "no library loaded");
        let load = HostRequest::Load {
            lib: "/nowhere/libnope.so".into(),
            mod_root: None,
        };
        assert!(host.handle(load).unwrap_err().contains("can't load"));
    }

    #[test]
    fn answers_are_found_past_what_the_mod_printed() {
        let mut out = b"the mod says hi\n".to_vec();
        out.extend_from_slice(MARK);
        yog_data::io::write_frame(&mut out, &HostReply::Err("no".into())).unwrap();
        let mut r = out.as_slice();
        assert_eq!(next_answer(&mut r).unwrap().unwrap().unwrap_err(), "no");
        assert!(next_answer(&mut r).unwrap().is_none());
    }

    /// `YOG_DESIGNER_LIB=…/libmod.so YOG_DESIGNER_COMPONENT=StatBar [YOG_DESIGNER_PNG=out.png]
    /// cargo test --lib designer::host -- --ignored` (`YOG_DESIGNER_PROPS="k=v;k=v"`)
    #[test]
    #[ignore]
    fn renders_a_built_mod() {
        let (Ok(lib), Ok(name)) = (
            std::env::var(yog_ids::env::DESIGNER_LIB),
            std::env::var(yog_ids::env::DESIGNER_COMPONENT),
        ) else {
            return;
        };
        let props = std::env::var(yog_ids::env::DESIGNER_PROPS).unwrap_or_default();
        let props = props
            .split(';')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut host = Host {
            lib: None,
            mod_root: None,
        };
        let loaded = host.handle(HostRequest::Load {
            lib: lib.into(),
            mod_root: None,
        });
        eprintln!("{loaded:?}");
        let l = host.loaded().unwrap();
        let (w, h, ops) = l
            .render(&RenderReq {
                name: name.clone(),
                attrs: props,
                w: 0.0,
                h: 0.0,
                avail_w: 400.0,
                avail_h: 300.0,
            })
            .unwrap();
        eprintln!("{w}x{h}: {ops:?}");
        assert!(w > 0.0 && h > 0.0 && !ops.is_empty());
        if let Ok(out) = std::env::var(yog_ids::env::DESIGNER_PNG) {
            let res = super::super::assets::Resources::for_mod(None);
            let canvas = raster::rasterize(&ops, w.ceil() as u32, h.ceil() as u32, 6, &res);
            std::fs::write(&out, canvas.png().unwrap()).unwrap();
        }
    }
}
