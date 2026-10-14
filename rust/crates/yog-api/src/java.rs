//! Other mods — Yog's and Java ones — from a Yog mod: whether one is
//! loaded ([`crate::mod_loaded`]), and calls into a Java mod's API by
//! reflection ([`call`]).
//!
//! ```ignore
//! use yog_api::java;
//! use yog_api::yog_data::Value;
//! if yog_api::mod_loaded("pehkui") {
//!     let entity = Value::Ref(yog_api::yog_data::Ref::new("entity", uuid));
//!     let scale = java::call("virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()", &[entity])
//!         .and_then(|v| v.as_f64());
//! }
//! ```

use yog_abi::YogStr;
use yog_data::Value;

use crate::registry::global_api;

/// Whether mod `id` is loaded: a Yog mod, or a Java mod in Fabric's,
/// Forge's or NeoForge's mod list. Content and code for another mod —
/// an optional integration — go behind it.
pub fn mod_loaded(id: &str) -> bool {
    let api = global_api();
    if api.is_null() {
        return false;
    }
    unsafe { ((*api).is_mod_loaded)((*api).ctx, YogStr::from_str(id)) }
}

/// Calls a Java mod's API: `expr` is a chain from a class —
/// `virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()` —
/// fields and methods, `$n` the `n`-th of `args`. Numbers, strings,
/// booleans and null cross as they are; an entity is
/// `Value::Ref(Ref::new("entity", uuid))` both ways. `None` when the call
/// failed (the log says why). For other mods' APIs only: Minecraft's own
/// classes are renamed at run time on some loaders.
pub fn call(expr: &str, args: &[Value]) -> Option<Value> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    let args = yog_data::text::to_string(&Value::List(args.to_vec()));
    let owned = unsafe { ((*api).java_call)((*api).ctx, YogStr::from_str(expr), YogStr::from_str(&args)) };
    let text = crate::registry::take_owned(unsafe { (*api).free_str }, owned)?;
    yog_data::text::parse_value(&text).ok()
}
