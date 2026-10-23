package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;
import dev.yog.content.YogAttributes;

import java.util.UUID;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.player.Player;

/** The host's entity actions: the methods {@code yog_event::entity} names. */
public final class YogEntityActions {
    private YogEntityActions() {}

    // --- entity ops (universal, by UUID) ---

    @YogServiceHost
    public static boolean entityTeleport(String uuid, double x, double y, double z) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        if (e instanceof ServerPlayer p) {
            p.connection.teleport(x, y, z, p.getYRot(), p.getXRot());
        } else if (e.level() instanceof ServerLevel sw) {
            e.teleportTo(sw, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot(), true);
        }
        return true;
    }

    @YogServiceHost
    public static double entityHealth(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        return e instanceof LivingEntity le ? le.getHealth() : Double.NaN;
    }

    @YogServiceHost
    public static boolean entitySetHealth(String uuid, double health) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e instanceof LivingEntity le) {
            le.setHealth((float) health);
            return true;
        }
        return false;
    }

    @YogServiceHost
    public static boolean entitySetVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.setDeltaMovement(vx, vy, vz);
        e.hurtMarked = true;
        return true;
    }

    @YogServiceHost
    public static boolean entityAddVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.push(vx, vy, vz);
        e.hurtMarked = true;
        return true;
    }

    @YogServiceHost
    public static boolean entityTeleportToDim(String uuid, String dimension, double x, double y, double z) {
        Entity e = NativeBridge.entityByUuid(uuid);
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (e == null || w == null) return false;
        if (e instanceof ServerPlayer p) {
            p.teleportTo(w, x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        } else {
            e.teleportTo(w, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot(), true);
        }
        return true;
    }

    @YogServiceHost
    public static boolean entityAddEffect(
            String uuid, String effectId, int durationTicks, int amplifier, boolean showParticles) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !BuiltInRegistries.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = BuiltInRegistries.MOB_EFFECT.getValue(id);
        if (effect == null) return false;
        return le.addEffect(new MobEffectInstance(
                BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect), durationTicks, amplifier, false, showParticles));
    }

    @YogServiceHost
    public static boolean entityRemoveEffect(String uuid, String effectId) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !BuiltInRegistries.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = BuiltInRegistries.MOB_EFFECT.getValue(id);
        if (effect == null) return false;
        return le.removeEffect(BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect));
    }

    @YogServiceHost
    public static boolean entityClearEffects(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        return le.removeAllEffects();
    }

    @YogServiceHost
    public static boolean entityKill(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        if (!(e.level() instanceof ServerLevel w)) return false;
        e.kill(w);
        return true;
    }

    @YogServiceHost
    public static String entityGetNbt(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return null;
        TagValueOutput out = TagValueOutput.createWithContext(ProblemReporter.DISCARDING, e.registryAccess());
        e.saveWithoutId(out);
        return out.buildResult().toString();
    }

    @YogServiceHost
    public static boolean entitySetNbt(String uuid, String snbt) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            e.load(TagValueInput.create(ProblemReporter.DISCARDING, e.registryAccess(), nbt));
            return true;
        } catch (Exception ex) {
            return false;
        }
    }

    @YogServiceHost
    public static double entityAttributeGet(String uuid, String attributeId) {
        AttributeInstance inst = YogAttributes.of(NativeBridge.entityByUuid(uuid), attributeId);
        return inst == null ? Double.NaN : inst.getBaseValue();
    }

    /** An attribute's value: its base changed by every modifier (effects, equipment). */
    @YogServiceHost
    public static double entityAttributeValue(String uuid, String attributeId) {
        AttributeInstance inst = YogAttributes.of(NativeBridge.entityByUuid(uuid), attributeId);
        return inst == null ? Double.NaN : inst.getValue();
    }

    @YogServiceHost
    public static boolean entityAttributeSet(String uuid, String attributeId, double value) {
        AttributeInstance inst = YogAttributes.of(NativeBridge.entityByUuid(uuid), attributeId);
        if (inst == null) return false;
        inst.setBaseValue(value);
        return true;
    }

    @YogServiceHost
    public static String spawnEntity(String typeId, String dimension, double x, double y, double z) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        Identifier id = Identifier.tryParse(typeId);
        if (w == null || id == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(id)) {
            return null;
        }
        EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.getValue(id);
        Entity e = type.create(w, EntitySpawnReason.COMMAND);
        if (e == null) {
            return null;
        }
        e.snapTo(x, y, z, e.getYRot(), e.getXRot());
        w.addFreshEntity(e);
        return e.getStringUUID();
    }

    /** Damage an entity with a damage type id (empty: from the attacker), caused by attacker (uuid or ""). */
    @YogServiceHost
    public static boolean entityDamage(String uuid, float amount, String damageType, String attackerUuid) {
        try {
            Entity e = NativeBridge.entityByUuid(uuid);
            if (e == null) return false;
            Entity src = NativeBridge.optionalEntity(attackerUuid);
            DamageSource ds = null;
            Identifier tid = damageType == null || damageType.isEmpty() ? null : Identifier.tryParse(damageType);
            if (tid != null) {
                var type = e.level().registryAccess().lookupOrThrow(Registries.DAMAGE_TYPE)
                        .get(ResourceKey.create(Registries.DAMAGE_TYPE, tid));
                if (type.isPresent()) ds = new DamageSource(type.get(), src);
            }
            if (ds == null) {
                if (tid == null && src instanceof Player sp) {
                    ds = e.damageSources().playerAttack(sp);
                } else if (tid == null && src instanceof LivingEntity sl) {
                    ds = e.damageSources().mobAttack(sl);
                } else {
                    ds = e.damageSources().magic();
                }
            }
            return e.level() instanceof ServerLevel w && e.hurtServer(w, ds, amount);
        } catch (Throwable t) {
            return false;
        }
    }

    @YogServiceHost
    public static boolean entitySetFireTicks(String uuid, int ticks) {
        try {
            Entity e = NativeBridge.entityByUuid(uuid);
            if (e == null) return false;
            e.setRemainingFireTicks(ticks);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** The UUID of what an entity rides, or null. */
    @YogServiceHost
    public static String entityVehicle(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        Entity v = e == null ? null : e.getVehicle();
        return v == null ? null : v.getStringUUID();
    }

    /** Seats {@code rider} on {@code vehicle}; {@code force}: even one that wouldn't take it. */
    @YogServiceHost
    public static boolean entityMount(String rider, String vehicle, boolean force) {
        Entity r = NativeBridge.entityByUuid(rider), v = NativeBridge.entityByUuid(vehicle);
        return r != null && v != null && r.startRiding(v, force, true);
    }

    /** Sets how far the entity has fallen (fall damage counts it on landing). */
    @YogServiceHost
    public static boolean entitySetFallDistance(String uuid, float distance) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.fallDistance = distance;
        return true;
    }

    /** Gets an entity off what it rides. */
    @YogServiceHost
    public static boolean entityDismount(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null || !e.isPassenger()) return false;
        e.stopRiding();
        return true;
    }
}
