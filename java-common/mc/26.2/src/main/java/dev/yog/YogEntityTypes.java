package dev.yog;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogContent;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
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

    private static final Map<EntityType<?>, YogTypes.EntityDef> DEFS = new HashMap<>();
    /** A hanging thing's type's box, blocks (a painting's); its own box is its size on the wall. */
    private static final float HANGING_BOX = 0.5F;
    private static final Map<String, EntityType<?>> TYPES = new LinkedHashMap<>();

    /** The definition of a mod's type, or null. */
    public static YogTypes.EntityDef def(EntityType<?> type) {
        return DEFS.get(type);
    }

    private static MobCategory category(String name) {
        try {
            return MobCategory.valueOf(name.toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException e) {
            return MobCategory.MISC;
        }
    }

    /** Every mod's type, with {@link YogRegistrar}; their attributes go to the loader's hook ({@link #attributes}). */
    static void schedule(List<YogTypes.EntityDef> defs) {
        YogRegistrar.add(Registries.ENTITY_TYPE, sink -> register(defs, sink));
    }

    @SuppressWarnings({"rawtypes"}) // a type built at runtime: its entity class isn't known statically
    private static void register(List<YogTypes.EntityDef> defs, YogRegistrar.Sink sink) {
        for (YogTypes.EntityDef def : defs) {
            Identifier id = Identifier.tryParse(def.id());
            if (id == null || TYPES.containsKey(def.id())) continue;
            EntityType.Builder b = YogContent.living(def)
                    ? EntityType.Builder.<YogMob>of(YogMob::new, category(def.category()))
                    : YogContent.hanging(def)
                    ? EntityType.Builder.<YogHanging>of(YogHanging::new, category(def.category()))
                    : EntityType.Builder.<YogObject>of(YogObject::new, category(def.category()));
            b = b.sized(YogContent.hanging(def) ? HANGING_BOX : def.width(), YogContent.hanging(def) ? HANGING_BOX : def.height());
            if (def.eyeHeight() != null) b = b.eyeHeight(def.eyeHeight());
            // A hanging thing doesn't move: as a painting, it's sent once.
            b = b.clientTrackingRange((int) def.trackingRange()).updateInterval(YogContent.hanging(def) ? Integer.MAX_VALUE : (int) def.updateInterval());
            if (def.fireImmune()) b = b.fireImmune();
            EntityType<?> type = b.build(ResourceKey.create(Registries.ENTITY_TYPE, id));
            DEFS.put(type, def);
            TYPES.put(def.id(), type);
            sink.register(Registries.ENTITY_TYPE, id, type);
        }
    }

    /** How a loader gives a living type its default attributes. */
    public interface Attributes {
        void put(EntityType<? extends Mob> type, AttributeSupplier.Builder attributes);
    }

    /** The living types' attributes, to the loader's hook (the attributes are registered by then). */
    @SuppressWarnings("unchecked") // the mod's entity types are of the entity classes their defs name
    public static void attributes(Attributes out) {
        for (EntityType<?> type : TYPES.values()) {
            YogTypes.EntityDef def = DEFS.get(type);
            if (YogContent.living(def)) out.put((EntityType<? extends Mob>) type, attributes(def));
        }
    }

    /** Spawn eggs of the living types that have one ({@code <id>_spawn_egg}), with {@link YogRegistrar}. */
    static void scheduleEggs() {
        YogRegistrar.add(Registries.ITEM, YogEntityTypes::eggs);
    }

    private static void eggs(YogRegistrar.Sink sink) {
        for (Map.Entry<String, EntityType<?>> t : TYPES.entrySet()) {
            YogTypes.EntityDef def = DEFS.get(t.getValue());
            if (def.spawnEgg() == null || !YogContent.living(def)) continue;
            Identifier id = Identifier.tryParse(dev.yog.data.YogIds.spawnEgg(def.id()));
            if (id == null) continue;
            sink.register(Registries.ITEM, id,
                    new SpawnEggItem(new Item.Properties().setId(ResourceKey.create(Registries.ITEM, id)).spawnEgg(t.getValue())));
        }
    }

    /** Every mod type, for the client's renderers. */
    public static Iterable<EntityType<?>> types() {
        return TYPES.values();
    }

    private static void add(AttributeSupplier.Builder a, String name, double value) {
        for (String id : dev.yog.data.YogIds.attributeCandidates(name)) {
            Identifier rl = Identifier.tryParse(id);
            var entry = rl == null ? java.util.Optional.<net.minecraft.core.Holder.Reference<Attribute>>empty() : BuiltInRegistries.ATTRIBUTE.get(rl);
            if (entry.isPresent()) {
                a.add(entry.get(), value);
                return;
            }
        }
        YogLog.warn("entity attribute " + name + ": no such attribute");
    }

    private static AttributeSupplier.Builder attributes(YogTypes.EntityDef def) {
        AttributeSupplier.Builder a = Mob.createMobAttributes();
        Map<String, Double> attrs = new LinkedHashMap<>(def.attributes());
        if (def.goals().contains("melee_attack")) attrs.putIfAbsent("attack_damage", 2.0);
        attrs.forEach((name, value) -> add(a, name, value));
        return a;
    }

    /** A tick of a mod's entity: to the mod, if it asked. */
    public static void tick(Entity e) {
        YogTypes.EntityDef def = DEFS.get(e.getType());
        if (def != null && def.ticks() && !e.level().isClientSide()) {
            new dev.yog.bus.YogEvents.Entity.Game.Tick(def.id(), e.getStringUUID()).emit();
        }
    }
}
