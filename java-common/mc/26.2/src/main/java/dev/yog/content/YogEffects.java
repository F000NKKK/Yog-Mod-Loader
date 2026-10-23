package dev.yog.content;

import dev.yog.NativeBridge;
import dev.yog.YogIds;
import dev.yog.YogLog;
import dev.yog.YogPlatform;

import dev.yog.bus.YogTypes;
import java.util.function.Consumer;
import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectCategory;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.alchemy.Potion;
import net.minecraft.world.item.alchemy.PotionContents;

/**
 * The mods' attributes (every player has them), mob effects (changing
 * attributes while they last) and potions (and their brewing) — from the
 * content blob, registered with the game.
 */
public final class YogEffects {
    private YogEffects() {}

    /** The mods' attributes: every player has them (the loader adds them to the player's defaults). */
    public static final List<Holder<Attribute>> PLAYER_ATTRIBUTES = new ArrayList<>();

    /** How a loader adds a brewing mix: {@code from} with {@code with} makes {@code to}. */
    public interface Mixes {
        void addMix(Holder<Potion> from, Item with, Holder<Potion> to);
    }

    /** The mods' attributes, effects and potions, with {@link YogRegistrar}. */
    static void schedule(YogContent content) {
        YogRegistrar.add(Registries.ATTRIBUTE, sink -> {
            for (YogTypes.AttributeDef a : content.attributes) {
                Identifier id = Identifier.parse(a.id());
                Attribute attr = new RangedAttribute(YogIds.attributeNameKey(a.id()), a.default_(), a.min(), a.max()).setSyncable(true);
                sink.register(Registries.ATTRIBUTE, id, attr);
                PLAYER_ATTRIBUTES.add(YogPlatform.holder(Registries.ATTRIBUTE, id));
            }
        });
        YogRegistrar.add(Registries.MOB_EFFECT, sink -> {
            for (YogTypes.EffectDef e : content.effects) {
                Identifier id = Identifier.parse(e.id());
                MobEffect effect = new Effect(category(e.category()), (int) e.color());
                for (YogTypes.EffectModifier m : e.modifiers()) {
                    effect.addAttributeModifier(YogPlatform.holder(Registries.ATTRIBUTE, Identifier.parse(m.attribute())), id, m.amount(), operation(m.operation()));
                }
                sink.register(Registries.MOB_EFFECT, id, effect);
            }
        });
        YogRegistrar.add(Registries.POTION, sink -> {
            for (YogTypes.PotionDef p : content.potions) {
                Identifier id = potionId(p.id());
                if (YogIds.isPotionAlias(p.id()) || BuiltInRegistries.POTION.containsKey(id)) continue;
                List<MobEffectInstance> effects = new ArrayList<>();
                for (YogTypes.PotionEffect pe : p.effects()) {
                    effects.add(new MobEffectInstance(YogPlatform.holder(Registries.MOB_EFFECT, Identifier.parse(pe.effect())), (int) pe.duration(), (int) pe.amplifier()));
                }
                sink.register(Registries.POTION, id, new Potion(p.name(), effects.toArray(new MobEffectInstance[0])));
            }
        });
    }

    /** The mods' brewing mixes, to the loader's brewing hook (the registries are filled by then). */
    public static void mixes(Mixes out) {
        for (YogTypes.PotionDef p : YogContent.cached(NativeBridge::nativeContent).potions) {
            var to = BuiltInRegistries.POTION.get(potionId(p.id()));
            for (YogTypes.Brew b : p.brewed()) {
                var from = BuiltInRegistries.POTION.get(potionId(b.from()));
                Identifier with = Identifier.parse(b.with());
                if (from.isEmpty() || to.isEmpty() || !BuiltInRegistries.ITEM.containsKey(with)) {
                    YogLog.warn("potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                    continue;
                }
                out.addMix(from.get(), BuiltInRegistries.ITEM.getValue(with), to.get());
            }
        }
    }

    private static Identifier potionId(String id) {
        return Identifier.parse(YogIds.gamePotionId(id));
    }

    /** What a potion comes in, in the order the game lists them. */
    private static final Item[] POTION_CONTAINERS = {Items.POTION, Items.SPLASH_POTION, Items.LINGERING_POTION, Items.TIPPED_ARROW};

    /** Namespace {@code namespace}'s potions — as a potion, splash and lingering, a tipped arrow — for its creative tab. */
    static void addTabStacks(String namespace, Consumer<ItemStack> out) {
        for (YogTypes.PotionDef p : YogContent.cached(NativeBridge::nativeContent).potions) {
            Identifier id = potionId(p.id());
            if (YogIds.isPotionAlias(p.id()) || !id.getNamespace().equals(namespace)) continue;
            BuiltInRegistries.POTION.get(id).ifPresent(potion -> {
                for (Item container : POTION_CONTAINERS) out.accept(PotionContents.createItemStack(container, potion));
            });
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
            case YogTypes.ModifierOperation.MULTIPLY_BASE -> AttributeModifier.Operation.ADD_MULTIPLIED_BASE;
            case YogTypes.ModifierOperation.MULTIPLY_TOTAL -> AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL;
            default -> AttributeModifier.Operation.ADD_VALUE;
        };
    }

    /** A mob effect of a mod: what it does is its attributes' modifiers. */
    private static final class Effect extends MobEffect {
        Effect(MobEffectCategory category, int color) {
            super(category, color);
        }
    }
}
