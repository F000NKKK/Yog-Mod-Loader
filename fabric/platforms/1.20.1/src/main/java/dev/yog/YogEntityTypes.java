package dev.yog;

import dev.yog.data.YogContent;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.entity.attribute.DefaultAttributeContainer;
import net.minecraft.entity.attribute.EntityAttribute;
import net.minecraft.entity.mob.MobEntity;
import net.minecraft.item.Item;
import net.minecraft.item.SpawnEggItem;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;

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

    private static SpawnGroup category(String name) {
        try {
            return SpawnGroup.valueOf(name.toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException e) {
            return SpawnGroup.MISC;
        }
    }

    /** Registers every mod's type, with its attributes. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void register(List<YogContent.Entity> defs) {
        for (YogContent.Entity def : defs) {
            Identifier id = Identifier.tryParse(def.id());
            if (id == null || TYPES.containsKey(def.id())) continue;
            EntityType.Builder b = def.living()
                    ? EntityType.Builder.<YogMob>create(YogMob::new, category(def.category()))
                    : def.hanging()
                    ? EntityType.Builder.<YogHanging>create(YogHanging::new, category(def.category()))
                    : EntityType.Builder.<YogObject>create(YogObject::new, category(def.category()));
            b = b.setDimensions(def.hanging() ? HANGING_BOX : def.width(), def.hanging() ? HANGING_BOX : def.height());
            // A hanging thing doesn't move: as a painting, it's sent once.
            b = b.maxTrackingRange(def.trackingRange()).trackingTickInterval(def.hanging() ? Integer.MAX_VALUE : def.updateInterval());
            if (def.fireImmune()) b = b.makeFireImmune();
            EntityType<?> type = b.build(def.id());
            DEFS.put(type, def);
            TYPES.put(def.id(), type);
            Registry.register(Registries.ENTITY_TYPE, id, type);
            if (def.living()) FabricDefaultAttributeRegistry.register((EntityType<? extends MobEntity>) type, attributes(def));
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
            Registry.register(Registries.ITEM, id,
                    new SpawnEggItem((EntityType<? extends MobEntity>) t.getValue(), def.spawnEgg()[0], def.spawnEgg()[1], new Item.Settings()));
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

    private static void add(DefaultAttributeContainer.Builder a, String name, double value) {
        for (String id : candidates(name)) {
            Identifier rl = Identifier.tryParse(id);
            EntityAttribute attribute = rl == null ? null : Registries.ATTRIBUTE.get(rl);
            if (attribute != null) {
                a.add(attribute, value);
                return;
            }
        }
        YogLog.warn("entity attribute " + name + ": no such attribute");
    }

    private static DefaultAttributeContainer.Builder attributes(YogContent.Entity def) {
        DefaultAttributeContainer.Builder a = MobEntity.createMobAttributes();
        Map<String, Double> attrs = new LinkedHashMap<>(def.attributes());
        if (def.goals().contains("melee_attack")) attrs.putIfAbsent("attack_damage", 2.0);
        attrs.forEach((name, value) -> add(a, name, value));
        return a;
    }

    /** A tick of a mod's entity: to the mod, if it asked. */
    public static void tick(Entity e) {
        YogContent.Entity def = DEFS.get(e.getType());
        if (def != null && def.ticks() && !e.getWorld().isClient) {
            NativeBridge.nativeOnEntityTick(def.id(), e.getUuidAsString());
        }
    }
}
