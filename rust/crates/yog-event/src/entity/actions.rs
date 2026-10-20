//! Entities: the host's actions.

use crate::Point;
use yog_core::EntityInfo;
use yog_ids as ids;

actions! {
    /// Moves the entity in its dimension.
    Teleport { uuid: String, pos: Point } -> bool = ids::service_entity::TELEPORT, "entityTeleport";
    /// Moves the entity to `pos` in `dim`.
    TeleportToDim { uuid: String, dim: String, pos: Point } -> bool = ids::service_entity::TELEPORT_TO_DIM, "entityTeleportToDim";
    /// The entity's position.
    Position { uuid: String } -> Option<Point> = ids::service_entity::POSITION, "YogQueries.entityPosition";
    /// Which way the entity faces.
    Rotation { uuid: String } -> Option<yog_core::Rotation> = ids::service_entity::ROTATION, "YogQueries.entityRotation";
    /// The entity's velocity.
    Velocity { uuid: String } -> Option<Point> = ids::service_entity::VELOCITY, "YogQueries.entityVelocity";
    /// Sets the entity's velocity.
    SetVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::SET_VELOCITY, "entitySetVelocity";
    /// Adds to the entity's velocity.
    AddVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::ADD_VELOCITY, "entityAddVelocity";
    /// A living entity's health.
    Health { uuid: String } -> Option<f64> = ids::service_entity::HEALTH, "entityHealth";
    /// Sets a living entity's health.
    SetHealth { uuid: String, health: f64 } -> bool = ids::service_entity::SET_HEALTH, "entitySetHealth";
    /// Kills the entity.
    Kill { uuid: String } -> bool = ids::service_entity::KILL, "entityKill";
    /// Spawns an entity of `entity_type`; its uuid.
    Spawn { entity_type: String, dim: String, pos: Point } -> Option<String> = ids::service_entity::SPAWN, "spawnEntity";
    /// Gives the entity a mob effect.
    AddEffect { uuid: String, effect: String, duration: i32, amplifier: u8, particles: bool } -> bool = ids::service_entity::ADD_EFFECT, "entityAddEffect";
    /// Takes a mob effect off the entity.
    RemoveEffect { uuid: String, effect: String } -> bool = ids::service_entity::REMOVE_EFFECT, "entityRemoveEffect";
    /// Takes every mob effect off the entity.
    ClearEffects { uuid: String } -> bool = ids::service_entity::CLEAR_EFFECTS, "entityClearEffects";
    /// The entity's data as SNBT.
    Nbt { uuid: String } -> Option<String> = ids::service_entity::NBT, "entityGetNbt";
    /// Merges `snbt` into the entity's data.
    SetNbt { uuid: String, snbt: String } -> bool = ids::service_entity::SET_NBT, "entitySetNbt";
    /// An attribute's base value.
    AttributeBase { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_BASE, "entityAttributeGet";
    /// Sets an attribute's base value.
    SetAttributeBase { uuid: String, attribute: String, value: f64 } -> bool = ids::service_entity::SET_ATTRIBUTE_BASE, "entityAttributeSet";
    /// An attribute's value with its modifiers.
    AttributeValue { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_VALUE, "entityAttributeValue";
    /// The entity's state.
    Info { uuid: String } -> Option<EntityInfo> = ids::service_entity::INFO, "YogQueries.entityInfo";
    /// Hurts the entity.
    Damage { uuid: String, amount: f32, damage_type: String, attacker: String } -> bool = ids::service_entity::DAMAGE, "entityDamage";
    /// Sets the entity on fire for `ticks`.
    SetFireTicks { uuid: String, ticks: i32 } -> bool = ids::service_entity::SET_FIRE_TICKS, "entitySetFireTicks";
    /// The uuid of what the entity rides.
    Vehicle { uuid: String } -> Option<String> = ids::service_entity::VEHICLE, "entityVehicle";
    /// The uuids riding the entity.
    Passengers { uuid: String } -> Vec<String> = ids::service_entity::PASSENGERS, "YogQueries.entityPassengers";
    /// Makes `rider` ride `vehicle`.
    Mount { rider: String, vehicle: String, force: bool } -> bool = ids::service_entity::MOUNT, "entityMount";
    /// Gets the entity off what it rides.
    Dismount { uuid: String } -> bool = ids::service_entity::DISMOUNT, "entityDismount";
    /// Sets how far the entity has fallen.
    SetFallDistance { uuid: String, distance: f32 } -> bool = ids::service_entity::SET_FALL_DISTANCE, "entitySetFallDistance";
}
