//! Mods' exported functions on the bus (`docs/design/bus.md`, stage 4):
//! `#[yog_export] fn name(..)` of mod `id` is a service at `id:name`; an
//! `import!` (or a mod's generated `*_exports` crate) calls it. The
//! arguments cross as a map of the parameters' names, the answer as its
//! own Yog Data — so a Java mod can call it too, and the bus can describe it.

use yog_abi::{YogBytes, YogHandlerFn, YogReply, YogStatus, status};
use yog_data::Schema;
use yog_event::Message;

use crate::registry::guard;
use crate::service::call_at;

/// A `#[yog_export]` function, as its `ctor` registers it.
#[doc(hidden)]
pub struct YogExportEntry {
    pub name: &'static str,
    pub handler: YogHandlerFn,
    /// Its doc comment.
    pub doc: &'static str,
    /// Its arguments' and its answer's shapes.
    pub shapes: fn() -> (yog_data::Shape, yog_data::Shape),
    /// The interfaces it implements (`#[yog_export(implements = "yog:energy/storage")]`).
    pub implements: &'static [&'static str],
}

#[doc(hidden)]
pub fn __yog_export_registry() -> &'static yog_sync::Mutex<Vec<YogExportEntry>> {
    static REG: yog_sync::Mutex<Vec<YogExportEntry>> = yog_sync::Mutex::new(Vec::new());
    &REG
}

/// Where mod `mod_id`'s export `name` is provided: `mod_id:name`.
pub fn export_address(mod_id: &str, name: &str) -> String {
    yog_ids::join(mod_id, name)
}

/// What an exported function's provider does: its arguments `A` read from
/// `payload`, `run` called, its answer `R` written to `reply`. A panic in
/// the mod is a failed call, not a crash.
///
/// # Safety
/// `payload` and `reply` are what the bus hands a handler (valid for the call).
#[doc(hidden)]
pub unsafe fn __yog_serve<A: Schema, R: Schema>(payload: YogBytes, reply: *const YogReply, run: impl FnOnce(A) -> R) -> YogStatus {
    let Some(args) = A::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let Some(answer) = guard(None, || Some(run(args))) else {
        return status::FAILED;
    };
    unsafe { YogReply::send(reply, &answer.to_bytes()) };
    status::OK
}

/// Calls mod `mod_id`'s export `name` with `args`: its answer, `None` when
/// it isn't there (the mod isn't loaded) or failed.
pub fn call_export<A: Schema, R: Schema>(mod_id: &str, name: &str, args: &A) -> Option<R> {
    R::from_bytes(&call_at(yog_event::resolve(&export_address(mod_id, name)), &args.to_bytes())?)
}

impl crate::Registry {
    /// Provides `mod_id`'s export `entry` (what `export_mod!` does for each).
    #[doc(hidden)]
    pub fn provide_export(&mut self, mod_id: &str, entry: &YogExportEntry) {
        let address = export_address(mod_id, entry.name);
        if self.provide_raw(&address, entry.handler) {
            let (args, answer) = (entry.shapes)();
            let interfaces = entry.implements.iter().map(|i| (*i).to_owned()).collect();
            self.declare(&address, entry.doc, &args, &answer, interfaces);
        }
    }
}
