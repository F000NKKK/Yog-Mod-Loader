//! Entities: the host's actions.

use crate::Point;
use yog_core::EntityInfo;
use yog_ids as ids;

actions! {
    /// Moves the entity in its dimension.
    Teleport { uuid: String, pos: Point } -> bool = ids::service_entity::TELEPORT, "actions.YogEntityActions.entityTeleport";
    /// Moves the entity to `pos` in `dim`.
    TeleportToDim { uuid: String, dim: String, pos: Point } -> bool = ids::service_entity::TELEPORT_TO_DIM, "actions.YogEntityActions.entityTeleportToDim";
    /// The entity's position.
    Position { uuid: String } -> Option<Point> = ids::service_entity::POSITION, "actions.YogQueries.entityPosition";
    /// Which way the entity faces.
    Rotation { uuid: String } -> Option<yog_core::Rotation> = ids::service_entity::ROTATION, "actions.YogQueries.entityRotation";
    /// The entity's velocity.
    Velocity { uuid: String } -> Option<Point> = ids::service_entity::VELOCITY, "actions.YogQueries.entityVelocity";
    /// Sets the entity's velocity.
    SetVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::SET_VELOCITY, "actions.YogEntityActions.entitySetVelocity";
    /// Adds to the entity's velocity.
    AddVelocity { uuid: String, velocity: Point } -> bool = ids::service_entity::ADD_VELOCITY, "actions.YogEntityActions.entityAddVelocity";
    /// A living entity's health.
    Health { uuid: String } -> Option<f64> = ids::service_entity::HEALTH, "actions.YogEntityActions.entityHealth";
    /// Sets a living entity's health.
    SetHealth { uuid: String, health: f64 } -> bool = ids::service_entity::SET_HEALTH, "actions.YogEntityActions.entitySetHealth";
    /// Kills the entity.
    Kill { uuid: String } -> bool = ids::service_entity::KILL, "actions.YogEntityActions.entityKill";
    /// Spawns an entity of `entity_type`; its uuid.
    Spawn { entity_type: String, dim: String, pos: Point } -> Option<String> = ids::service_entity::SPAWN, "actions.YogEntityActions.spawnEntity";
    /// Gives the entity a mob effect.
    AddEffect { uuid: String, effect: String, duration: i32, amplifier: u8, particles: bool } -> bool = ids::service_entity::ADD_EFFECT, "actions.YogEntityActions.entityAddEffect";
    /// Takes a mob effect off the entity.
    RemoveEffect { uuid: String, effect: String } -> bool = ids::service_entity::REMOVE_EFFECT, "actions.YogEntityActions.entityRemoveEffect";
    /// Takes every mob effect off the entity.
    ClearEffects { uuid: String } -> bool = ids::service_entity::CLEAR_EFFECTS, "actions.YogEntityActions.entityClearEffects";
    /// The entity's data as SNBT.
    Nbt { uuid: String } -> Option<String> = ids::service_entity::NBT, "actions.YogEntityActions.entityGetNbt";
    /// Merges `snbt` into the entity's data.
    SetNbt { uuid: String, snbt: String } -> bool = ids::service_entity::SET_NBT, "actions.YogEntityActions.entitySetNbt";
    /// An attribute's base value.
    AttributeBase { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_BASE, "actions.YogEntityActions.entityAttributeGet";
    /// Sets an attribute's base value.
    SetAttributeBase { uuid: String, attribute: String, value: f64 } -> bool = ids::service_entity::SET_ATTRIBUTE_BASE, "actions.YogEntityActions.entityAttributeSet";
    /// An attribute's value with its modifiers.
    AttributeValue { uuid: String, attribute: String } -> Option<f64> = ids::service_entity::ATTRIBUTE_VALUE, "actions.YogEntityActions.entityAttributeValue";
    /// The entity's state.
    Info { uuid: String } -> Option<EntityInfo> = ids::service_entity::INFO, "actions.YogQueries.entityInfo";
    /// Hurts the entity.
    Damage { uuid: String, amount: f32, damage_type: String, attacker: String } -> bool = ids::service_entity::DAMAGE, "actions.YogEntityActions.entityDamage";
    /// Sets the entity on fire for `ticks`.
    SetFireTicks { uuid: String, ticks: i32 } -> bool = ids::service_entity::SET_FIRE_TICKS, "actions.YogEntityActions.entitySetFireTicks";
    /// The uuid of what the entity rides.
    Vehicle { uuid: String } -> Option<String> = ids::service_entity::VEHICLE, "actions.YogEntityActions.entityVehicle";
    /// The uuids riding the entity.
    Passengers { uuid: String } -> Vec<String> = ids::service_entity::PASSENGERS, "actions.YogQueries.entityPassengers";
    /// Makes `rider` ride `vehicle`.
    Mount { rider: String, vehicle: String, force: bool } -> bool = ids::service_entity::MOUNT, "actions.YogEntityActions.entityMount";
    /// Gets the entity off what it rides.
    Dismount { uuid: String } -> bool = ids::service_entity::DISMOUNT, "actions.YogEntityActions.entityDismount";
    /// Sets how far the entity has fallen.
    SetFallDistance { uuid: String, distance: f32 } -> bool = ids::service_entity::SET_FALL_DISTANCE, "actions.YogEntityActions.entitySetFallDistance";
}
