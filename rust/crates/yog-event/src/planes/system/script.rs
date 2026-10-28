//! Yog Script mods' classes on the JVM: the host's actions.

use yog_ids as ids;

actions! {
    /// Mod `mod_id`'s scripts reloaded from its rebuilt `.yog` at `path`
    /// (hot reload): its live objects' state kept, its old classes' handlers
    /// gone, its new classes loaded; whether they loaded.
    Reload { mod_id: String, path: String } -> bool = ids::service_script::RELOAD, "script.YsMods.reload";
}
