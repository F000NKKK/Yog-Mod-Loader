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
