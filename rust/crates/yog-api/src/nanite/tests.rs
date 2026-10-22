//! What `#[nanite]` makes of a class and its members.

use crate::nanite as yog_nanite;
use yog_nanite::{CONSTRUCTOR, Nanite};

#[crate::nanite("net.minecraft.world.damagesource.DamageSource")]
pub struct DamageSource;

#[crate::nanite("net.minecraft.world.entity.LivingEntity")]
pub struct LivingEntity;

#[crate::nanite]
impl LivingEntity {
    pub fn hurt(&self, source: &DamageSource, amount: f32) -> bool;
    #[field]
    pub fn health(&self) -> f32;
    #[new]
    pub fn make(source: &DamageSource, ticks: i32) -> Option<Self>;
    pub fn named(id: i64) -> Option<String>;
    pub fn tick(&self);
}

#[test]
fn members_get_their_names_and_descriptors() {
    assert_eq!(
        LivingEntity::CLASS,
        "net.minecraft.world.entity.LivingEntity"
    );
    assert_eq!(
        __YOG_NANITE_LIVINGENTITY_HURT.signature(),
        (
            "hurt",
            "(Lnet/minecraft/world/damagesource/DamageSource;F)Z".to_owned()
        )
    );
    assert_eq!(
        __YOG_NANITE_LIVINGENTITY_HEALTH.signature(),
        ("health", "F".to_owned())
    );
    assert_eq!(
        __YOG_NANITE_LIVINGENTITY_MAKE.signature(),
        (
            CONSTRUCTOR,
            "(Lnet/minecraft/world/damagesource/DamageSource;I)V".to_owned()
        )
    );
    assert_eq!(
        __YOG_NANITE_LIVINGENTITY_NAMED.signature(),
        ("named", "(J)Ljava/lang/String;".to_owned())
    );
    assert_eq!(
        __YOG_NANITE_LIVINGENTITY_TICK.signature(),
        ("tick", "()V".to_owned())
    );
}

#[test]
fn an_entity_is_reached_by_its_uuid() {
    let e = LivingEntity::of(&yog_event::Entity::new("u-1"));
    assert_eq!(
        e.handle().to_value(),
        yog_data::Value::Ref(yog_data::Ref::new(yog_ids::kind::ENTITY, "u-1"))
    );
}

/// Stops a hurt over 10.
#[crate::inject(LivingEntity::HURT, at = HEAD, cancellable, priority = 5)]
fn shield(hook: &mut yog_nanite::Hook) {
    if hook.arg::<f32>(1).is_some_and(|amount| amount > 10.0) {
        hook.set_return(false);
    }
}

#[test]
fn an_injection_is_declared_before_the_game_loads() {
    let declared: yog_event::nanite::Nanites =
        yog_data::read_bytes(&yog_nanite::__yog_nanites()).unwrap();
    let hurt = declared
        .injects
        .iter()
        .find(|i| i.method == "hurt")
        .unwrap();
    assert_eq!(hurt.class, LivingEntity::CLASS);
    assert_eq!(
        hurt.desc,
        "(Lnet/minecraft/world/damagesource/DamageSource;F)Z"
    );
    assert_eq!(
        (hurt.at.as_str(), hurt.cancellable),
        (yog_ids::mixin_at::HEAD, true)
    );
    assert_eq!(
        hurt.point,
        "net.minecraft.world.entity.LivingEntity#hurt(Lnet/minecraft/world/damagesource/DamageSource;F)Z@HEAD"
    );
    let entry = yog_nanite::__yog_nanite_registry().lock();
    let mut e = yog_event::nanite::Injected {
        args: vec![yog_data::Value::Null, yog_data::Value::Float(12.0)],
        ..Default::default()
    };
    (entry[0].handler)(&mut yog_nanite::Hook::new(&mut e));
    assert!(e.cancelled && e.returned && e.value == yog_data::Value::Bool(false));
}
