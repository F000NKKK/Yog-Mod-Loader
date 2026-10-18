package dev.yog;

import java.util.function.Consumer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.alchemy.PotionContents;
import dev.yog.data.YogContent;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectCategory;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.alchemy.Potion;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.brewing.RegisterBrewingRecipesEvent;
import net.neoforged.neoforge.event.entity.EntityAttributeModificationEvent;
import net.neoforged.neoforge.registries.RegisterEvent;

/**
 * The mods' attributes (every player has them), mob effects (changing
 * attributes while they last) and potions (and their brewing) — from the
 * content blob, registered with the game. Effects are registered before
 * potions and attributes after both, so the attributes an effect changes
 * are added to it once every registry is filled (common setup).
 */
final class YogEffects {
    private YogEffects() {}

    private static final Map<String, Attribute> ATTRIBUTES = new LinkedHashMap<>();
    private static final Map<String, MobEffect> EFFECTS = new LinkedHashMap<>();

    /** Listens on the mod bus (registries, the player's attributes, setup) and the game bus (brewing). */
    static void listen(IEventBus modBus) {
        modBus.addListener(YogEffects::onRegister);
        modBus.addListener(YogEffects::onPlayerAttributes);
        modBus.addListener(YogEffects::onSetup);
        NeoForge.EVENT_BUS.addListener(YogEffects::onBrewing);
    }

    private static YogContent content() {
        return YogContent.cached(NativeBridge::nativeContent);
    }

    private static ResourceLocation id(String id) {
        return ResourceLocation.parse(id);
    }

    private static ResourceLocation potionId(String id) {
        return ResourceLocation.parse(YogIds.gamePotionId(id));
    }

    /** What a potion comes in, in the order the game lists them. */
    private static final Item[] POTION_CONTAINERS = {Items.POTION, Items.SPLASH_POTION, Items.LINGERING_POTION, Items.TIPPED_ARROW};

    /** Namespace {@code namespace}'s potions — as a potion, splash and lingering, a tipped arrow — for its creative tab. */
    static void addTabStacks(String namespace, Consumer<ItemStack> out) {
        for (YogContent.Potion p : YogContent.cached(NativeBridge::nativeContent).potions) {
            ResourceLocation id = potionId(p.id());
            if (YogIds.isPotionAlias(p.id()) || !id.getNamespace().equals(namespace)) continue;
            BuiltInRegistries.POTION.getHolder(ResourceKey.create(Registries.POTION, id)).ifPresent(potion -> {
                for (Item container : POTION_CONTAINERS) out.accept(PotionContents.createItemStack(container, potion));
            });
        }
    }

    private static void onRegister(RegisterEvent event) {
        if (event.getRegistryKey().equals(Registries.MOB_EFFECT)) {
            for (YogContent.Effect e : content().effects) {
                MobEffect effect = new Effect(category(e.category()), e.color());
                EFFECTS.put(e.id(), effect);
                event.register(Registries.MOB_EFFECT, id(e.id()), () -> effect);
            }
        } else if (event.getRegistryKey().equals(Registries.POTION)) {
            for (YogContent.Potion p : content().potions) {
                java.util.List<MobEffectInstance> effects = new java.util.ArrayList<>();
                for (YogContent.PotionEffect pe : p.effects()) {
                    var effect = BuiltInRegistries.MOB_EFFECT.getHolder(ResourceKey.create(Registries.MOB_EFFECT, id(pe.effect())));
                    if (effect.isEmpty()) {
                        YogLog.warn("potion " + p.id() + ": no effect " + pe.effect());
                        continue;
                    }
                    effects.add(new MobEffectInstance(effect.get(), pe.duration(), pe.amplifier()));
                }
                Potion potion = new Potion(p.name(), effects.toArray(new MobEffectInstance[0]));
                if (!YogIds.isPotionAlias(p.id())) event.register(Registries.POTION, id(p.id()), () -> potion);
            }
        } else if (event.getRegistryKey().equals(Registries.ATTRIBUTE)) {
            for (YogContent.Attribute a : content().attributes) {
                ResourceLocation rl = id(a.id());
                Attribute attr = new RangedAttribute(YogIds.attributeNameKey(a.id()),
                        a.defaultValue(), a.min(), a.max()).setSyncable(true);
                ATTRIBUTES.put(a.id(), attr);
                event.register(Registries.ATTRIBUTE, rl, () -> attr);
            }
        }
    }

    private static void onPlayerAttributes(EntityAttributeModificationEvent event) {
        for (Attribute attr : ATTRIBUTES.values()) {
            event.add(EntityType.PLAYER, BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attr));
        }
    }

    private static void onSetup(FMLCommonSetupEvent event) {
        event.enqueueWork(() -> {
            for (YogContent.Effect e : content().effects) {
                MobEffect effect = EFFECTS.get(e.id());
                if (effect == null) continue;
                for (YogContent.Modifier m : e.modifiers()) {
                    var attr = BuiltInRegistries.ATTRIBUTE.getHolder(ResourceKey.create(Registries.ATTRIBUTE, id(m.attribute())));
                    if (attr.isEmpty()) {
                        YogLog.warn("effect " + e.id() + ": no attribute " + m.attribute());
                        continue;
                    }
                    effect.addAttributeModifier(attr.get(), id(e.id()), m.amount(), operation(m.operation()));
                }
            }
        });
    }

    private static void onBrewing(RegisterBrewingRecipesEvent event) {
        for (YogContent.Potion p : content().potions) {
            var to = BuiltInRegistries.POTION.getHolder(ResourceKey.create(Registries.POTION, potionId(p.id())));
            for (YogContent.Brew b : p.brewed()) {
                var from = BuiltInRegistries.POTION.getHolder(ResourceKey.create(Registries.POTION, potionId(b.from())));
                ResourceLocation with = id(b.with());
                if (from.isEmpty() || to.isEmpty() || !BuiltInRegistries.ITEM.containsKey(with)) {
                    YogLog.warn("potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                    continue;
                }
                event.getBuilder().addMix(from.get(), BuiltInRegistries.ITEM.get(with), to.get());
            }
        }
    }

    private static MobEffectCategory category(YogContent.EffectCategory c) {
        return switch (c) {
            case BENEFICIAL -> MobEffectCategory.BENEFICIAL;
            case HARMFUL -> MobEffectCategory.HARMFUL;
            case NEUTRAL -> MobEffectCategory.NEUTRAL;
        };
    }

    private static AttributeModifier.Operation operation(YogContent.Operation o) {
        return switch (o) {
            case MULTIPLY_BASE -> AttributeModifier.Operation.ADD_MULTIPLIED_BASE;
            case MULTIPLY_TOTAL -> AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL;
            case ADD -> AttributeModifier.Operation.ADD_VALUE;
        };
    }

    /** A mob effect of a mod: what it does is its attributes' modifiers. */
    private static final class Effect extends MobEffect {
        Effect(MobEffectCategory category, int color) {
            super(category, color);
        }
    }
}
