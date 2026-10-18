package dev.yog;

import java.util.function.Consumer;
import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.List;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.fabricmc.fabric.api.registry.FabricPotionBrewingBuilder;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectCategory;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;
import net.minecraft.world.entity.player.Player;
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
final class YogEffects {
    private YogEffects() {}

    static void register(YogContent content) {
        List<Holder<Attribute>> attributes = new ArrayList<>();
        for (YogContent.Attribute a : content.attributes) {
            Identifier id = Identifier.parse(a.id());
            Attribute attr = new RangedAttribute(
                    YogIds.attributeNameKey(a.id()), a.defaultValue(), a.min(), a.max())
                    .setSyncable(true);
            attributes.add(Registry.registerForHolder(BuiltInRegistries.ATTRIBUTE, id, attr));
        }
        if (!attributes.isEmpty()) {
            AttributeSupplier.Builder player = Player.createAttributes();
            attributes.forEach(player::add);
            FabricDefaultAttributeRegistry.register(net.minecraft.world.entity.EntityTypes.PLAYER, player);
        }

        for (YogContent.Effect e : content.effects) {
            Identifier id = Identifier.parse(e.id());
            MobEffect effect = new Effect(category(e.category()), e.color());
            for (YogContent.Modifier m : e.modifiers()) {
                var attr = BuiltInRegistries.ATTRIBUTE.get(Identifier.parse(m.attribute()));
                if (attr.isEmpty()) {
                    YogLog.warn("effect " + id + ": no attribute " + m.attribute());
                    continue;
                }
                effect.addAttributeModifier(attr.get(), id, m.amount(), operation(m.operation()));
            }
            Registry.register(BuiltInRegistries.MOB_EFFECT, id, effect);
        }

        for (YogContent.Potion p : content.potions) {
            List<MobEffectInstance> effects = new ArrayList<>();
            for (YogContent.PotionEffect pe : p.effects()) {
                var effect = BuiltInRegistries.MOB_EFFECT.get(Identifier.parse(pe.effect()));
                if (effect.isEmpty()) {
                    YogLog.warn("potion " + p.id() + ": no effect " + pe.effect());
                    continue;
                }
                effects.add(new MobEffectInstance(effect.get(), pe.duration(), pe.amplifier()));
            }
            Identifier id = potionId(p.id());
            if (!YogIds.isPotionAlias(p.id()) && !BuiltInRegistries.POTION.containsKey(id)) {
                Registry.register(BuiltInRegistries.POTION, id, new Potion(p.name(), effects.toArray(new MobEffectInstance[0])));
            }
        }
        FabricPotionBrewingBuilder.BUILD.register(builder -> {
            for (YogContent.Potion p : content.potions) {
                var to = BuiltInRegistries.POTION.get(potionId(p.id()));
                for (YogContent.Brew b : p.brewed()) {
                    var from = BuiltInRegistries.POTION.get(potionId(b.from()));
                    Identifier with = Identifier.parse(b.with());
                    if (from.isEmpty() || to.isEmpty() || !BuiltInRegistries.ITEM.containsKey(with)) {
                        YogLog.warn("potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                        continue;
                    }
                    builder.addMix(from.get(), BuiltInRegistries.ITEM.getValue(with), to.get());
                }
            }
        });
    }

    private static Identifier potionId(String id) {
        return Identifier.parse(YogIds.gamePotionId(id));
    }

    /** What a potion comes in, in the order the game lists them. */
    private static final Item[] POTION_CONTAINERS = {Items.POTION, Items.SPLASH_POTION, Items.LINGERING_POTION, Items.TIPPED_ARROW};

    /** Namespace {@code namespace}'s potions — as a potion, splash and lingering, a tipped arrow — for its creative tab. */
    static void addTabStacks(String namespace, Consumer<ItemStack> out) {
        for (YogContent.Potion p : YogContent.cached(NativeBridge::nativeContent).potions) {
            Identifier id = potionId(p.id());
            if (YogIds.isPotionAlias(p.id()) || !id.getNamespace().equals(namespace)) continue;
            BuiltInRegistries.POTION.get(id).ifPresent(potion -> {
                for (Item container : POTION_CONTAINERS) out.accept(PotionContents.createItemStack(container, potion));
            });
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
