package dev.yog.content;

import net.minecraft.core.Registry;
import dev.yog.YogLog;
import dev.yog.YogObject;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogContent;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.function.BiConsumer;
import net.minecraft.resources.ResourceLocation;
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
 * their attributes and spawn eggs (their renderer: {@link YogClientEvents#renderers}).
 */
public final class YogEntityTypes {
    private YogEntityTypes() {}

    private static final Map<EntityType<?>, YogTypes.EntityDef> DEFS = new HashMap<>();
    /** A hanging thing's type's box, blocks (a painting's). */
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
        YogRegistrar.add(Registry.ENTITY_TYPE_REGISTRY, sink -> build(defs, (id, type) -> sink.register(Registry.ENTITY_TYPE_REGISTRY, id, type)));
    }

    @SuppressWarnings({"unchecked", "rawtypes"}) // a type built at runtime: its entity class isn't known statically
    private static void build(List<YogTypes.EntityDef> defs, BiConsumer<ResourceLocation, EntityType<?>> register) {
        for (YogTypes.EntityDef def : defs) {
            ResourceLocation id = ResourceLocation.tryParse(def.id());
            if (id == null || TYPES.containsKey(def.id())) continue;
            EntityType.Builder b = YogContent.living(def)
                    ? EntityType.Builder.<YogMob>of(YogMob::new, category(def.category()))
                    : YogContent.hanging(def)
                    ? EntityType.Builder.<YogHanging>of(YogHanging::new, category(def.category()))
                    : EntityType.Builder.<YogObject>of(YogObject::new, category(def.category()));
            // A hanging thing's box is its size on the wall, its type's a painting's; it doesn't move, so it's sent once.
            b = YogContent.hanging(def)
                    ? b.sized(HANGING_BOX, HANGING_BOX).clientTrackingRange((int) def.trackingRange()).updateInterval(Integer.MAX_VALUE)
                    : b.sized(def.width(), def.height()).clientTrackingRange((int) def.trackingRange()).updateInterval((int) def.updateInterval());
            if (def.fireImmune()) b = b.fireImmune();
            EntityType<?> type = b.build(def.id());
            DEFS.put(type, def);
            TYPES.put(def.id(), type);
            register.accept(id, type);
        }
    }

    /** Spawn eggs of the living types that have one ({@code <id>_spawn_egg}), with {@link YogRegistrar}. */
    static void scheduleEggs() {
        YogRegistrar.add(Registry.ITEM_REGISTRY, sink -> eggs((id, item) -> sink.register(Registry.ITEM_REGISTRY, id, item)));
    }

    @SuppressWarnings("unchecked") // the mod's entity types are of the entity classes their defs name
    private static void eggs(BiConsumer<ResourceLocation, Item> register) {
        for (Map.Entry<String, EntityType<?>> t : TYPES.entrySet()) {
            YogTypes.EntityDef def = DEFS.get(t.getValue());
            if (def.spawnEgg() == null || !YogContent.living(def)) continue;
            ResourceLocation id = ResourceLocation.tryParse(dev.yog.data.YogIds.spawnEgg(def.id()));
            if (id == null) continue;
            register.accept(id, new SpawnEggItem((EntityType<? extends Mob>) t.getValue(), (int) def.spawnEgg()[0], (int) def.spawnEgg()[1], new Item.Properties()));
        }
    }

    private static void add(AttributeSupplier.Builder a, String name, double value) {
        for (String id : dev.yog.data.YogIds.attributeCandidates(name)) {
            ResourceLocation rl = ResourceLocation.tryParse(id);
            Attribute attribute = rl == null ? null : Registry.ATTRIBUTE.get(rl);
            if (attribute != null) {
                a.add(attribute, value);
                return;
            }
        }
        YogLog.warn("entity attribute " + name + ": no such attribute");
    }

    private static AttributeSupplier attributes(YogTypes.EntityDef def) {
        AttributeSupplier.Builder a = Mob.createMobAttributes();
        Map<String, Double> attrs = new LinkedHashMap<>(def.attributes());
        if (def.goals().contains("melee_attack")) attrs.putIfAbsent("attack_damage", 2.0);
        attrs.forEach((name, value) -> add(a, name, value));
        return a.build();
    }

    /** A tick of a mod's entity: to the mod, if it asked. */
    public static void tick(Entity e) {
        YogTypes.EntityDef def = DEFS.get(e.getType());
        if (def != null && def.ticks() && !e.getLevel().isClientSide) {
            new dev.yog.bus.YogEvents.Entity.Game.Tick(def.id(), e.getStringUUID()).emit();
        }
    }

    /** How a loader gives a living type its default attributes. */
    public interface Attributes {
        void put(EntityType<? extends net.minecraft.world.entity.LivingEntity> type, AttributeSupplier attributes);
    }

    /** The living types' attributes, to the loader's hook (the attributes are registered by then). */
    @SuppressWarnings("unchecked") // the mod's entity types are of the entity classes their defs name
    public static void attributes(Attributes out) {
        DEFS.forEach((type, def) -> {
            if (YogContent.living(def)) out.put((EntityType<? extends net.minecraft.world.entity.LivingEntity>) type, attributes(def));
        });
    }

    /** Every mod type, for the client's renderers. */
    public static Iterable<EntityType<?>> types() {
        return TYPES.values();
    }
}
