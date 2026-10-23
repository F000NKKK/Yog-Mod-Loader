//! Java mods' code, reached by reflection: the host's actions.

use yog_data::Value;
use yog_ids as ids;

actions! {
    /// A chain from a class (`virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()`)
    /// called with `args` (`$n`); its result (`None`: failed, or null).
    Call { expr: String, args: Vec<Value> } -> Option<Value> = ids::service_jvm::CALL, "data.YogJava.invoke";
}
