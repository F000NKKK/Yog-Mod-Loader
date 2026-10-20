//! JNI: spawns, placing, deaths, advancements, interactions, crafting, explosions.

use super::*;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemCraft<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    result_item: JString<'l>,
    result_count: jint,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.item_craft.is_empty() {
        return;
    }
    let (p, pu, ri, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, result_item),
        jstr!(env, dimension),
    );
    let ev = YogCraftEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        result_item: YogStr::from_str(&ri),
        result_count: result_count as u32,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_item_craft", || {
        each(&h.item_craft, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

