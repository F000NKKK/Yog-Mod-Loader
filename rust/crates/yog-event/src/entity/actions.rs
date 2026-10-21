//! Entities: the host's actions.

use crate::Point;
use yog_core::EntityInfo;
use yog_ids as ids;

actions! {
    /// Moves the entity in its dimension.
    Teleport { uuid: String, pos: Point } -> bool = ids::service_entity::TELEPORT, "YogEntityActions.entityTeleport";
    /// Moves the entity to `pos` in `dim`.
    TeleportToDim { uuid: String, dim: String, pos: Point } -> bool = ids::service_entity::TELEPORT_TO_DIM, "YogEntityActions.entityTeleportToDim";
    /// The entity's position.
    Position { uuid: String } -> Option<Point> = ids::service_entity::POSITION, "YogQueries.entityPosition";
    /// Which way the entity faces.
    Rotation { uuid: String } -> Option<yog_core::Rotation> = ids::service_entity::ROTATION, "YogQueries.entityRotation";
    /// The entity's velocity.
    Velocity { uuid: String } -> Option<Point> = ids::service_entity::VELOCITY, "YogQueries.entityVelocity";
    /// Sets the entity's velocity.
    SetVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::SET_VELOCITY, "YogEntityActions.entitySetVelocity";
    /// Adds to the entity's velocity.
    AddVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::ADD_VELOCITY, "YogEntityActions.entityAddVelocity";
    /// A living entity's health.
    Health { uuid: String } -> Option<f64> = ids::service_entity::HEALTH, "YogEntityActions.entityHealth";
    /// Sets a living entity's health.
    SetHealth { uuid: String, health: f64 } -> bool = ids::service_entity::SET_HEALTH, "YogEntityActions.entitySetHealth";
    /// Kills the entity.
    Kill { uuid: String } -> bool = ids::service_entity::KILL, "YogEntityActions.entityKill";
    /// Spawns an entity of `entity_type`; its uuid.
    Spawn { entity_type: String, dim: String, pos: Point } -> Option<String> = ids::service_entity::SPAWN, "YogEntityActions.spawnEntity";
    /// Gives the entity a mob effect.
    AddEffect { uuid: String, effect: String, duration: i32, amplifier: u8, particles: bool } -> bool = ids::service_entity::ADD_EFFECT, "YogEntityActions.entityAddEffect";
    /// Takes a mob effect off the entity.
    RemoveEffect { uuid: String, effect: String } -> bool = ids::service_entity::REMOVE_EFFECT, "YogEntityActions.entityRemoveEffect";
    /// Takes every mob effect off the entity.
    ClearEffects { uuid: String } -> bool = ids::service_entity::CLEAR_EFFECTS, "YogEntityActions.entityClearEffects";
    /// The entity's data as SNBT.
    Nbt { uuid: String } -> Option<String> = ids::service_entity::NBT, "YogEntityActions.entityGetNbt";
    /// Merges `snbt` into the entity's data.
    SetNbt { uuid: String, snbt: String } -> bool = ids::service_entity::SET_NBT, "YogEntityActions.entitySetNbt";
    /// An attribute's base value.
    AttributeBase { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_BASE, "YogEntityActions.entityAttributeGet";
    /// Sets an attribute's base value.
    SetAttributeBase { uuid: String, attribute: String, value: f64 } -> bool = ids::service_entity::SET_ATTRIBUTE_BASE, "YogEntityActions.entityAttributeSet";
    /// An attribute's value with its modifiers.
    AttributeValue { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_VALUE, "YogEntityActions.entityAttributeValue";
    /// The entity's state.
    Info { uuid: String } -> Option<EntityInfo> = ids::service_entity::INFO, "YogQueries.entityInfo";
    /// Hurts the entity.
    Damage { uuid: String, amount: f32, damage_type: String, attacker: String } -> bool = ids::service_entity::DAMAGE, "YogEntityActions.entityDamage";
    /// Sets the entity on fire for `ticks`.
    SetFireTicks { uuid: String, ticks: i32 } -> bool = ids::service_entity::SET_FIRE_TICKS, "YogEntityActions.entitySetFireTicks";
    /// The uuid of what the entity rides.
    Vehicle { uuid: String } -> Option<String> = ids::service_entity::VEHICLE, "YogEntityActions.entityVehicle";
    /// The uuids riding the entity.
    Passengers { uuid: String } -> Vec<String> = ids::service_entity::PASSENGERS, "YogQueries.entityPassengers";
    /// Makes `rider` ride `vehicle`.
    Mount { rider: String, vehicle: String, force: bool } -> bool = ids::service_entity::MOUNT, "YogEntityActions.entityMount";
    /// Gets the entity off what it rides.
    Dismount { uuid: String } -> bool = ids::service_entity::DISMOUNT, "YogEntityActions.entityDismount";
    /// Sets how far the entity has fallen.
    SetFallDistance { uuid: String, distance: f32 } -> bool = ids::service_entity::SET_FALL_DISTANCE, "YogEntityActions.entitySetFallDistance";
}
