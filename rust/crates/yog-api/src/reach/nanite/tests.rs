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
    #[name("getLastHurtByMob")]
    pub fn last_hurt_by(&self) -> Option<LivingEntity>;
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
        yog_data::Value::Ref(yog_data::Ref::of::<yog_data::kinds::Entity>(
            &"u-1".to_owned()
        ))
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
        .find(|i| i.method == "hurt" && i.kind == yog_ids::nanite_kind::INJECT)
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
        "inject:net.minecraft.world.entity.LivingEntity#hurt(Lnet/minecraft/world/damagesource/DamageSource;F)Z@HEAD"
    );
    let entry = yog_nanite::__yog_nanite_registry().lock();
    let mut e = yog_event::nanite::Injected {
        args: vec![yog_data::Value::Null, yog_data::Value::Float(12.0)],
        ..Default::default()
    };
    let shield = entry
        .iter()
        .find(|e| e.kind == yog_ids::nanite_kind::INJECT)
        .unwrap();
    (shield.handler.unwrap())(&mut yog_nanite::Hook::new(&mut e));
    assert!(e.cancelled && e.returned && e.value == yog_data::Value::Bool(false));
}

/// Halves the amount a hurt hands on to `actuallyHurt`.
#[crate::modify_arg(
    LivingEntity::HURT,
    target = "Lnet/minecraft/world/entity/LivingEntity;actuallyHurt(Lnet/minecraft/world/damagesource/DamageSource;F)V",
    index = 1
)]
fn halve(hook: &mut yog_nanite::Hook) {
    let amount = hook.value::<f32>().unwrap_or_default();
    hook.set_value(amount / 2.0);
}

/// Answers a redirected call with its original's result.
#[crate::wrap(
    LivingEntity::HURT,
    target = "Lnet/minecraft/world/entity/LivingEntity;isInvulnerableTo(Lnet/minecraft/world/damagesource/DamageSource;)Z"
)]
fn as_was(hook: &mut yog_nanite::Hook) {
    let _ = hook.call_original(Vec::new());
}

#[test]
fn every_kind_has_its_point() {
    let declared: yog_event::nanite::Nanites =
        yog_data::read_bytes(&yog_nanite::__yog_nanites()).unwrap();
    let of = |kind: &str| {
        declared
            .injects
            .iter()
            .find(|i| i.kind == kind)
            .unwrap()
            .clone()
    };
    let arg = of(yog_ids::nanite_kind::MODIFY_ARG);
    assert_eq!((arg.at.as_str(), arg.index), (yog_ids::mixin_at::INVOKE, 1));
    assert!(
        arg.point.starts_with("modify_arg:") && arg.point.ends_with("V:1"),
        "{}",
        arg.point
    );
    let wrap = of(yog_ids::nanite_kind::REDIRECT);
    assert!(wrap.target.contains("isInvulnerableTo"));
    assert_ne!(arg.point, wrap.point);
}

crate::added_field!(LivingEntity, pub CHARGE: i32);
crate::implement!(LivingEntity, "net.minecraft.world.entity.OwnableEntity");

/// The charge, as an added method.
#[crate::add_method(LivingEntity, name = "yogCharge", desc = "()I")]
fn charge(hook: &mut yog_nanite::Hook) {
    hook.set_value(3i32);
}

#[test]
fn added_members_are_declared() {
    let declared: yog_event::nanite::Nanites =
        yog_data::read_bytes(&yog_nanite::__yog_nanites()).unwrap();
    let of = |kind: &str| {
        declared
            .injects
            .iter()
            .find(|i| i.kind == kind)
            .unwrap()
            .clone()
    };
    let field = of(yog_ids::nanite_kind::ADD_FIELD);
    assert_eq!(
        (field.method.as_str(), field.desc.as_str()),
        ("yog$yog-api$CHARGE", "I")
    );
    let iface = of(yog_ids::nanite_kind::IMPLEMENT);
    assert_eq!(iface.target, "net.minecraft.world.entity.OwnableEntity");
    let method = of(yog_ids::nanite_kind::ADD_METHOD);
    assert_eq!(
        (method.method.as_str(), method.desc.as_str()),
        ("yogCharge", "()I")
    );
    assert_eq!(method.class, LivingEntity::CLASS);
}

/// Every write of the health field, kept.
#[crate::redirect(LivingEntity::HURT, at = FIELD, target = "Lnet/minecraft/world/entity/LivingEntity;lastHurt:F", write)]
fn last_hurt(hook: &mut yog_nanite::Hook) {
    hook.set_value(0.0f32);
}

#[test]
fn a_field_redirect_tells_its_write() {
    let declared: yog_event::nanite::Nanites =
        yog_data::read_bytes(&yog_nanite::__yog_nanites()).unwrap();
    let write = declared.injects.iter().find(|i| i.write).unwrap();
    assert_eq!(
        (write.at.as_str(), write.kind.as_str()),
        (yog_ids::mixin_at::FIELD, yog_ids::nanite_kind::REDIRECT)
    );
    assert!(write.point.ends_with(":write"), "{}", write.point);
}
