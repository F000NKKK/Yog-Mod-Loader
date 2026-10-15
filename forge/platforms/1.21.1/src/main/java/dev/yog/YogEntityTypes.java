package dev.yog;

import dev.yog.data.YogContent;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.function.BiConsumer;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.SpawnEggItem;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.EntityRenderersEvent;
import net.minecraftforge.event.entity.EntityAttributeCreationEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

/**
 * Mods' entity types ({@code .yentity}): objects ({@link YogObject}),
 * living things and mobs ({@link YogMob}), their attributes, spawn eggs and
 * renderer ({@link YogEntityRenderer}).
 */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.MOD)
public final class YogEntityTypes {
    private YogEntityTypes() {}

    private static final Map<EntityType<?>, YogContent.Entity> DEFS = new HashMap<>();
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

    /** Builds every mod's type and hands it to {@code register}. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void build(List<YogContent.Entity> defs, BiConsumer<ResourceLocation, EntityType<?>> register) {
        for (YogContent.Entity def : defs) {
            ResourceLocation id = ResourceLocation.tryParse(def.id());
            if (id == null || TYPES.containsKey(def.id())) continue;
            EntityType.Builder b = def.living()
                    ? EntityType.Builder.<YogMob>of(YogMob::new, category(def.category()))
                    : EntityType.Builder.<YogObject>of(YogObject::new, category(def.category()));
            b = b.sized(def.width(), def.height()).clientTrackingRange(def.trackingRange()).updateInterval(def.updateInterval());
            if (def.eyeHeight() != null) b = b.eyeHeight(def.eyeHeight());
            if (def.fireImmune()) b = b.fireImmune();
            EntityType<?> type = b.build(def.id());
            DEFS.put(type, def);
            TYPES.put(def.id(), type);
            register.accept(id, type);
        }
    }

    /** Spawn eggs of the living types that have one ({@code <id>_spawn_egg}). */
    @SuppressWarnings("unchecked")
    public static void eggs(BiConsumer<ResourceLocation, Item> register) {
        for (Map.Entry<String, EntityType<?>> t : TYPES.entrySet()) {
            YogContent.Entity def = DEFS.get(t.getValue());
            if (def.spawnEgg() == null || !def.living()) continue;
            ResourceLocation id = ResourceLocation.tryParse(def.id() + "_spawn_egg");
            if (id == null) continue;
            register.accept(id, new SpawnEggItem((EntityType<? extends Mob>) t.getValue(), def.spawnEgg()[0], def.spawnEgg()[1], new Item.Properties()));
        }
    }

    /** {@code max_health} → the game's {@code generic.max_health}; a full id stays. */
    private static List<String> candidates(String name) {
        if (name.contains(":")) return List.of(name);
        return List.of("minecraft:" + name, "minecraft:generic." + name);
    }

    private static void add(AttributeSupplier.Builder a, String name, double value) {
        for (String id : candidates(name)) {
            ResourceLocation rl = ResourceLocation.tryParse(id);
            var holder = rl == null ? java.util.Optional.<net.minecraft.core.Holder.Reference<Attribute>>empty() : BuiltInRegistries.ATTRIBUTE.getHolder(rl);
            if (holder.isPresent()) {
                a.add(holder.get(), value);
                return;
            }
        }
        System.err.println("[yog] entity attribute " + name + ": no such attribute");
    }

    private static AttributeSupplier attributes(YogContent.Entity def) {
        AttributeSupplier.Builder a = Mob.createMobAttributes();
        Map<String, Double> attrs = new LinkedHashMap<>(def.attributes());
        if (def.goals().contains("melee_attack")) attrs.putIfAbsent("attack_damage", 2.0);
        attrs.forEach((name, value) -> add(a, name, value));
        return a.build();
    }

    /** A tick of a mod's entity: to the mod, if it asked. */
    public static void tick(Entity e) {
        YogContent.Entity def = DEFS.get(e.getType());
        if (def != null && def.ticks() && !e.level().isClientSide) {
            NativeBridge.nativeOnEntityTick(def.id(), e.getStringUUID());
        }
    }

    @SubscribeEvent
    @SuppressWarnings("unchecked")
    public static void onAttributes(EntityAttributeCreationEvent event) {
        DEFS.forEach((type, def) -> {
            if (def.living()) event.put((EntityType<? extends net.minecraft.world.entity.LivingEntity>) type, attributes(def));
        });
    }

    /** The renderer of every mod type. */
    @Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.MOD, value = Dist.CLIENT)
    public static final class Client {
        private Client() {}

        @SubscribeEvent
        @SuppressWarnings("unchecked")
        public static void onRenderers(EntityRenderersEvent.RegisterRenderers event) {
            for (EntityType<?> type : TYPES.values()) {
                event.registerEntityRenderer((EntityType<Entity>) type, YogEntityRenderer::new);
            }
        }
    }
}
