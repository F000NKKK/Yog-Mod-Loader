package dev.yog.actions;

import net.minecraft.core.Registry;
import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;
import dev.yog.content.YogAttributes;

import java.util.UUID;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.phys.Vec3;

/** The host's entity actions: the methods {@code yog_event::entity} names. */
public final class YogEntityActions {
    private YogEntityActions() {}

    // ── Entity ops ──────────────────────────────────────────────────────

    @YogServiceHost
    public static boolean entityTeleport(String uuid, double x, double y, double z) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.teleportTo(x, y, z);
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
        if (e instanceof LivingEntity le) { le.setHealth((float) health); return true; }
        return false;
    }

    @YogServiceHost
    public static boolean entitySetVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.setDeltaMovement(vx, vy, vz);
        e.hasImpulse = true;
        return true;
    }

    @YogServiceHost
    public static boolean entityAddVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.setDeltaMovement(e.getDeltaMovement().add(vx, vy, vz));
        e.hasImpulse = true;
        return true;
    }

    @YogServiceHost
    public static boolean entityTeleportToDim(String uuid, String dimension, double x, double y, double z) {
        Entity e = NativeBridge.entityByUuid(uuid);
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (e == null || w == null) return false;
        if (e instanceof net.minecraft.server.level.ServerPlayer sp) {
            sp.teleportTo(w, x, y, z, e.getYRot(), e.getXRot());
            return true;
        }
        Entity there = e.getLevel() == w ? e : e.changeDimension(w);
        if (there == null) return false;
        there.teleportTo(x, y, z);
        return true;
    }

    @YogServiceHost
    public static boolean entityAddEffect(String uuid, String effectId, int durationTicks, int amplifier, boolean showParticles) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        ResourceLocation id = ResourceLocation.tryParse(effectId);
        if (id == null || !Registry.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = Registry.MOB_EFFECT.get(id);
        return le.addEffect(new MobEffectInstance(effect, durationTicks, amplifier, false, showParticles));
    }

    @YogServiceHost
    public static boolean entityKill(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        e.kill();
        return true;
    }

    @YogServiceHost
    public static String entityGetNbt(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return null;
        CompoundTag nbt = new CompoundTag();
        e.save(nbt);
        return nbt.toString();
    }

    @YogServiceHost
    public static boolean entitySetNbt(String uuid, String snbt) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return false;
        try { CompoundTag nbt = TagParser.parseTag(snbt); e.load(nbt); return true; }
        catch (Exception ex) { return false; }
    }

    @YogServiceHost
    public static String spawnEntity(String typeId, String dimension, double x, double y, double z) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        ResourceLocation id = ResourceLocation.tryParse(typeId);
        if (w == null || id == null || !Registry.ENTITY_TYPE.containsKey(id)) return null;
        EntityType<?> type = Registry.ENTITY_TYPE.get(id);
        Entity e = type.create(w);
        if (e == null) return null;
        e.moveTo(x, y, z, e.getYRot(), e.getXRot());
        w.addFreshEntity(e);
        return e.getStringUUID();
    }

    /**
     * The damage source of type id: this version names its sources by
     * message id, the type's path in camel case ({@code in_fire} is
     * {@code inFire}) — the game's own source when it has one and no one
     * caused it, else one of that id caused by src.
     */
    private static DamageSource damageSource(ResourceLocation id, Entity src) {
        StringBuilder msg = new StringBuilder();
        for (String word : id.getPath().split(dev.yog.YogNames.Sep.WORD)) {
            msg.append(msg.isEmpty() ? word : Character.toUpperCase(word.charAt(0)) + word.substring(1));
        }
        String msgId = msg.toString();
        if (src != null) return new net.minecraft.world.damagesource.EntityDamageSource(msgId, src);
        for (java.lang.reflect.Field f : DamageSource.class.getFields()) {
            if (!java.lang.reflect.Modifier.isStatic(f.getModifiers()) || f.getType() != DamageSource.class) continue;
            try {
                if (f.get(null) instanceof DamageSource d && d.getMsgId().equals(msgId)) return d;
            } catch (IllegalAccessException ignored) {
                // A public field: always readable.
            }
        }
        return new DamageSource(msgId);
    }

    /** Damage an entity with a damage type id (empty: from the attacker), caused by attacker (uuid or ""). */
    @YogServiceHost
    public static boolean entityDamage(String uuid, float amount, String damageType, String attackerUuid) {
        try {
            Entity e = NativeBridge.entityByUuid(uuid);
            if (e == null) return false;
            Entity src = NativeBridge.optionalEntity(attackerUuid);
            DamageSource ds = null;
            ResourceLocation tid = damageType == null || damageType.isEmpty() ? null : ResourceLocation.tryParse(damageType);
            if (tid != null) {
                ds = damageSource(tid, src);
            } else if (src instanceof Player sp) {
                ds = DamageSource.playerAttack(sp);
            } else if (src instanceof LivingEntity sl) {
                ds = DamageSource.mobAttack(sl);
            } else {
                ds = DamageSource.MAGIC;
            }
            return e.hurt(ds, amount);
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

    // ── effects / loot / tags / particles / attributes ──────────────────────

    @YogServiceHost
    public static boolean entityRemoveEffect(String uuid, String effectId) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        ResourceLocation id = ResourceLocation.tryParse(effectId);
        if (id == null || !Registry.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = Registry.MOB_EFFECT.get(id);
        if (effect == null) return false;
        return le.removeEffect(effect);
    }

    @YogServiceHost
    public static boolean entityClearEffects(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        return le.removeAllEffects();
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
        return r != null && v != null && r.startRiding(v, force);
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
