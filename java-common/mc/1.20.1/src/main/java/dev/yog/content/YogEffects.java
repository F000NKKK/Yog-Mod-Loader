package dev.yog.content;

import dev.yog.NativeBridge;
import dev.yog.YogIds;
import dev.yog.YogLog;

import dev.yog.bus.YogTypes;
import java.util.function.Consumer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.alchemy.PotionUtils;
import dev.yog.data.YogContent;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectCategory;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.alchemy.Potion;

/**
 * The mods' attributes (every player has them), mob effects (changing
 * attributes while they last) and potions (and their brewing) — from the
 * content blob, registered with the game through {@link YogRegistrar}.
 * The attributes an effect changes, and the brewing, are added once every
 * registry is filled ({@link #linkModifiers}, the loader's common setup).
 */
public final class YogEffects {
    private YogEffects() {}

    private static final Map<String, Attribute> ATTRIBUTES = new LinkedHashMap<>();
    private static final Map<String, MobEffect> EFFECTS = new LinkedHashMap<>();

    /** The mods' attributes, effects and potions, with {@link YogRegistrar}. */
    static void schedule(YogContent content) {
        YogRegistrar.add(Registries.MOB_EFFECT, sink -> {
            for (YogTypes.EffectDef e : content.effects) {
                MobEffect effect = new Effect(category(e.category()), (int) e.color());
                EFFECTS.put(e.id(), effect);
                sink.register(Registries.MOB_EFFECT, id(e.id()), effect);
            }
        });
        YogRegistrar.add(Registries.POTION, sink -> {
            for (YogTypes.PotionDef p : content.potions) {
                java.util.List<MobEffectInstance> effects = new java.util.ArrayList<>();
                for (YogTypes.PotionEffect pe : p.effects()) {
                    MobEffect effect = BuiltInRegistries.MOB_EFFECT.get(id(pe.effect()));
                    if (effect == null) {
                        YogLog.warn("potion " + p.id() + ": no effect " + pe.effect());
                        continue;
                    }
                    effects.add(new MobEffectInstance(effect, (int) pe.duration(), (int) pe.amplifier()));
                }
                if (!YogIds.isPotionAlias(p.id())) sink.register(Registries.POTION, id(p.id()), new Potion(p.name(), effects.toArray(new MobEffectInstance[0])));
            }
        });
        YogRegistrar.add(Registries.ATTRIBUTE, sink -> {
            for (YogTypes.AttributeDef a : content.attributes) {
                Attribute attr = new RangedAttribute(YogIds.attributeNameKey(a.id()), a.default_(), a.min(), a.max()).setSyncable(true);
                ATTRIBUTES.put(a.id(), attr);
                sink.register(Registries.ATTRIBUTE, id(a.id()), attr);
            }
        });
    }

    /** The mods' attributes: every player has them (the loader adds them to the player's defaults). */
    public static java.util.Collection<Attribute> playerAttributes() {
        return ATTRIBUTES.values();
    }

    private static YogContent content() {
        return YogContent.cached(NativeBridge::nativeContent);
    }

    private static ResourceLocation id(String id) {
        return new ResourceLocation(id);
    }

    private static ResourceLocation potionId(String id) {
        return new ResourceLocation(YogIds.gamePotionId(id));
    }

    /** What a potion comes in, in the order the game lists them. */
    private static final Item[] POTION_CONTAINERS = {Items.POTION, Items.SPLASH_POTION, Items.LINGERING_POTION, Items.TIPPED_ARROW};

    /** Namespace {@code namespace}'s potions — as a potion, splash and lingering, a tipped arrow — for its creative tab. */
    static void addTabStacks(String namespace, Consumer<ItemStack> out) {
        for (YogTypes.PotionDef p : YogContent.cached(NativeBridge::nativeContent).potions) {
            ResourceLocation id = potionId(p.id());
            if (YogIds.isPotionAlias(p.id()) || !id.getNamespace().equals(namespace)) continue;
            BuiltInRegistries.POTION.getOptional(id).ifPresent(potion -> {
                for (Item container : POTION_CONTAINERS) out.accept(PotionUtils.setPotion(new ItemStack(container), potion));
            });
        }
    }

    /**
     * The attributes each effect changes, added to it, and the brewing mixes
     * — once every registry is filled (the loader's common setup): effects
     * come before attributes.
     */
    public static void linkModifiers() {
        {
            for (YogTypes.EffectDef e : content().effects) {
                MobEffect effect = EFFECTS.get(e.id());
                if (effect == null) continue;
                for (YogTypes.EffectModifier m : e.modifiers()) {
                    Attribute attr = BuiltInRegistries.ATTRIBUTE.get(id(m.attribute()));
                    if (attr == null) {
                        YogLog.warn("effect " + e.id() + ": no attribute " + m.attribute());
                        continue;
                    }
                    // A modifier's UUID: the same for an effect and attribute every run.
                    String uuid = java.util.UUID.nameUUIDFromBytes((e.id() + "/" + m.attribute())
                            .getBytes(java.nio.charset.StandardCharsets.UTF_8)).toString();
                    effect.addAttributeModifier(attr, uuid, m.amount(), operation(m.operation()));
                }
            }
            for (YogTypes.PotionDef p : content().potions) {
                Potion to = BuiltInRegistries.POTION.get(potionId(p.id()));
                for (YogTypes.Brew b : p.brewed()) {
                    ResourceLocation from = potionId(b.from()), with = id(b.with());
                    if (!BuiltInRegistries.POTION.containsKey(from) || !BuiltInRegistries.ITEM.containsKey(with)) {
                        YogLog.warn("potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                        continue;
                    }
                    dev.yog.mixin.PotionBrewingAccess.yog$addMix(BuiltInRegistries.POTION.get(from), BuiltInRegistries.ITEM.get(with), to);
                }
            }
        }
    }

    private static MobEffectCategory category(String c) {
        return switch (c) {
            case YogTypes.EffectCategory.BENEFICIAL -> MobEffectCategory.BENEFICIAL;
            case YogTypes.EffectCategory.HARMFUL -> MobEffectCategory.HARMFUL;
            default -> MobEffectCategory.NEUTRAL;
        };
    }

    private static AttributeModifier.Operation operation(String o) {
        return switch (o) {
            case YogTypes.ModifierOperation.MULTIPLY_BASE -> AttributeModifier.Operation.MULTIPLY_BASE;
            case YogTypes.ModifierOperation.MULTIPLY_TOTAL -> AttributeModifier.Operation.MULTIPLY_TOTAL;
            default -> AttributeModifier.Operation.ADDITION;
        };
    }

    /** A mob effect of a mod: what it does is its attributes' modifiers. */
    private static final class Effect extends MobEffect {
        Effect(MobEffectCategory category, int color) {
            super(category, color);
        }
    }
}
