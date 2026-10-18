package dev.yog;

import dev.yog.data.YogContent;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.SpawnEggItem;

/**
 * Mods' entity types ({@code .yentity}): objects ({@link YogObject}),
 * living things and mobs ({@link YogMob}), hanging things ({@link YogHanging}),
 * their attributes, spawn eggs and renderer ({@link YogEntityRenderer}).
 */
public final class YogEntityTypes {
    private YogEntityTypes() {}

    private static final Map<EntityType<?>, YogContent.Entity> DEFS = new HashMap<>();
    /** A hanging thing's type's box, blocks (a painting's); its own box is its size on the wall. */
    private static final float HANGING_BOX = 0.5F;
    private static final Map<String, EntityType<?>> TYPES = new LinkedHashMap<>();

    /** The definition of a mod's type, or null. */
    public static YogContent.Entity def(EntityType<?> type) {
        return DEFS.get(type);
    }

    private static MobCategory category(String name) {
        try {
            return MobCategory.valueOf(name.toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException e) {
            return MobCategory.MISC;
        }
    }

    /** Registers every mod's type, with its attributes. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void register(List<YogContent.Entity> defs) {
        for (YogContent.Entity def : defs) {
            Identifier id = Identifier.tryParse(def.id());
            if (id == null || TYPES.containsKey(def.id())) continue;
            EntityType.Builder b = def.living()
                    ? EntityType.Builder.<YogMob>of(YogMob::new, category(def.category()))
                    : def.hanging()
                    ? EntityType.Builder.<YogHanging>of(YogHanging::new, category(def.category()))
                    : EntityType.Builder.<YogObject>of(YogObject::new, category(def.category()));
            b = b.sized(def.hanging() ? HANGING_BOX : def.width(), def.hanging() ? HANGING_BOX : def.height());
            if (def.eyeHeight() != null) b = b.eyeHeight(def.eyeHeight());
            // A hanging thing doesn't move: as a painting, it's sent once.
            b = b.clientTrackingRange(def.trackingRange()).updateInterval(def.hanging() ? Integer.MAX_VALUE : def.updateInterval());
            if (def.fireImmune()) b = b.fireImmune();
            EntityType<?> type = b.build(ResourceKey.create(Registries.ENTITY_TYPE, id));
            DEFS.put(type, def);
            TYPES.put(def.id(), type);
            Registry.register(BuiltInRegistries.ENTITY_TYPE, id, type);
            if (def.living()) FabricDefaultAttributeRegistry.register((EntityType<? extends Mob>) type, attributes(def));
        }
    }

    /** Spawn eggs of the living types that have one ({@code <id>_spawn_egg}). */
    @SuppressWarnings("unchecked")
    public static void eggs() {
        for (Map.Entry<String, EntityType<?>> t : TYPES.entrySet()) {
            YogContent.Entity def = DEFS.get(t.getValue());
            if (def.spawnEgg() == null || !def.living()) continue;
            Identifier id = Identifier.tryParse(def.id() + "_spawn_egg");
            if (id == null) continue;
            Registry.register(BuiltInRegistries.ITEM, id,
                    new SpawnEggItem(new Item.Properties().setId(ResourceKey.create(Registries.ITEM, id)).spawnEgg(t.getValue())));
        }
    }

    /** Every mod type, for the client's renderers. */
    public static Iterable<EntityType<?>> types() {
        return TYPES.values();
    }

    /** {@code max_health} → the game's {@code generic.max_health}; a full id stays. */
    private static List<String> candidates(String name) {
        if (name.contains(":")) return List.of(name);
        return List.of("minecraft:" + name, "minecraft:generic." + name);
    }

    private static void add(AttributeSupplier.Builder a, String name, double value) {
        for (String id : candidates(name)) {
            Identifier rl = Identifier.tryParse(id);
            var entry = rl == null ? java.util.Optional.<net.minecraft.core.Holder.Reference<Attribute>>empty() : BuiltInRegistries.ATTRIBUTE.get(rl);
            if (entry.isPresent()) {
                a.add(entry.get(), value);
                return;
            }
        }
        YogLog.warn("entity attribute " + name + ": no such attribute");
    }

    private static AttributeSupplier.Builder attributes(YogContent.Entity def) {
        AttributeSupplier.Builder a = Mob.createMobAttributes();
        Map<String, Double> attrs = new LinkedHashMap<>(def.attributes());
        if (def.goals().contains("melee_attack")) attrs.putIfAbsent("attack_damage", 2.0);
        attrs.forEach((name, value) -> add(a, name, value));
        return a;
    }

    /** A tick of a mod's entity: to the mod, if it asked. */
    public static void tick(Entity e) {
        YogContent.Entity def = DEFS.get(e.getType());
        if (def != null && def.ticks() && !e.level().isClientSide()) {
            NativeBridge.nativeOnEntityTick(def.id(), e.getStringUUID());
        }
    }
}
