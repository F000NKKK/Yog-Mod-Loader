package dev.yog;

import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.List;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.fabricmc.fabric.api.registry.FabricBrewingRecipeRegistryBuilder;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.attribute.ClampedEntityAttribute;
import net.minecraft.entity.attribute.DefaultAttributeContainer;
import net.minecraft.entity.attribute.EntityAttribute;
import net.minecraft.entity.attribute.EntityAttributeModifier;
import net.minecraft.entity.effect.StatusEffect;
import net.minecraft.entity.effect.StatusEffectCategory;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.potion.Potion;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.registry.entry.RegistryEntry;
import net.minecraft.util.Identifier;

/**
 * The mods' attributes (every player has them), mob effects (changing
 * attributes while they last) and potions (and their brewing) — from the
 * content blob, registered with the game.
 */
final class YogEffects {
    private YogEffects() {}

    static void register(YogContent content) {
        List<RegistryEntry<EntityAttribute>> attributes = new ArrayList<>();
        for (YogContent.Attribute a : content.attributes) {
            Identifier id = Identifier.of(a.id());
            EntityAttribute attr = new ClampedEntityAttribute(
                    "attribute.name." + id.getNamespace() + "." + id.getPath(), a.defaultValue(), a.min(), a.max())
                    .setTracked(true);
            attributes.add(Registry.registerReference(Registries.ATTRIBUTE, id, attr));
        }
        if (!attributes.isEmpty()) {
            DefaultAttributeContainer.Builder player = PlayerEntity.createPlayerAttributes();
            attributes.forEach(player::add);
            FabricDefaultAttributeRegistry.register(EntityType.PLAYER, player);
        }

        for (YogContent.Effect e : content.effects) {
            Identifier id = Identifier.of(e.id());
            StatusEffect effect = new Effect(category(e.category()), e.color());
            for (YogContent.Modifier m : e.modifiers()) {
                var attr = Registries.ATTRIBUTE.getEntry(Identifier.of(m.attribute()));
                if (attr.isEmpty()) {
                    System.err.println("[yog] effect " + id + ": no attribute " + m.attribute());
                    continue;
                }
                effect.addAttributeModifier(attr.get(), id, m.amount(), operation(m.operation()));
            }
            Registry.register(Registries.STATUS_EFFECT, id, effect);
        }

        for (YogContent.Potion p : content.potions) {
            List<StatusEffectInstance> effects = new ArrayList<>();
            for (YogContent.PotionEffect pe : p.effects()) {
                var effect = Registries.STATUS_EFFECT.getEntry(Identifier.of(pe.effect()));
                if (effect.isEmpty()) {
                    System.err.println("[yog] potion " + p.id() + ": no effect " + pe.effect());
                    continue;
                }
                effects.add(new StatusEffectInstance(effect.get(), pe.duration(), pe.amplifier()));
            }
            Registry.register(Registries.POTION, Identifier.of(p.id()), new Potion(p.name(), effects.toArray(new StatusEffectInstance[0])));
        }
        FabricBrewingRecipeRegistryBuilder.BUILD.register(builder -> {
            for (YogContent.Potion p : content.potions) {
                var to = Registries.POTION.getEntry(Identifier.of(p.id()));
                for (YogContent.Brew b : p.brewed()) {
                    var from = Registries.POTION.getEntry(potionId(b.from()));
                    Identifier with = Identifier.of(b.with());
                    if (from.isEmpty() || to.isEmpty() || !Registries.ITEM.containsId(with)) {
                        System.err.println("[yog] potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                        continue;
                    }
                    builder.registerPotionRecipe(from.get(), Registries.ITEM.get(with), to.get());
                }
            }
        });
    }

    private static Identifier potionId(String id) {
        return id.startsWith("yog:") ? Identifier.of("minecraft", id.substring(4)) : Identifier.of(id);
    }

    private static StatusEffectCategory category(String c) {
        return switch (c) {
            case "beneficial" -> StatusEffectCategory.BENEFICIAL;
            case "harmful" -> StatusEffectCategory.HARMFUL;
            default -> StatusEffectCategory.NEUTRAL;
        };
    }

    private static EntityAttributeModifier.Operation operation(String o) {
        return switch (o) {
            case "multiply_base" -> EntityAttributeModifier.Operation.ADD_MULTIPLIED_BASE;
            case "multiply_total" -> EntityAttributeModifier.Operation.ADD_MULTIPLIED_TOTAL;
            default -> EntityAttributeModifier.Operation.ADD_VALUE;
        };
    }

    /** A mob effect of a mod: what it does is its attributes' modifiers. */
    private static final class Effect extends StatusEffect {
        Effect(StatusEffectCategory category, int color) {
            super(category, color);
        }
    }
}
