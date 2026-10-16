package dev.yog;

import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.List;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.fabricmc.fabric.api.registry.FabricBrewingRecipeRegistry;
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
import net.minecraft.util.Identifier;

/**
 * The mods' attributes (every player has them), mob effects (changing
 * attributes while they last) and potions (and their brewing) — from the
 * content blob, registered with the game.
 */
final class YogEffects {
    private YogEffects() {}

    static void register(YogContent content) {
        List<EntityAttribute> attributes = new ArrayList<>();
        for (YogContent.Attribute a : content.attributes) {
            Identifier id = new Identifier(a.id());
            EntityAttribute attr = new ClampedEntityAttribute(
                    "attribute.name." + id.getNamespace() + "." + id.getPath(), a.defaultValue(), a.min(), a.max())
                    .setTracked(true);
            attributes.add(Registry.register(Registries.ATTRIBUTE, id, attr));
        }
        if (!attributes.isEmpty()) {
            DefaultAttributeContainer.Builder player = PlayerEntity.createPlayerAttributes();
            attributes.forEach(player::add);
            FabricDefaultAttributeRegistry.register(EntityType.PLAYER, player);
        }

        for (YogContent.Effect e : content.effects) {
            Identifier id = new Identifier(e.id());
            StatusEffect effect = new Effect(category(e.category()), e.color());
            for (YogContent.Modifier m : e.modifiers()) {
                var attr = Registries.ATTRIBUTE.getEntry(net.minecraft.registry.RegistryKey.of(net.minecraft.registry.RegistryKeys.ATTRIBUTE, new Identifier(m.attribute())));
                if (attr.isEmpty()) {
                    System.err.println("[yog] effect " + id + ": no attribute " + m.attribute());
                    continue;
                }
                // A modifier's UUID: the same for an effect and attribute every run.
                String uuid = java.util.UUID.nameUUIDFromBytes((id + "/" + m.attribute()).getBytes(java.nio.charset.StandardCharsets.UTF_8)).toString();
                effect.addAttributeModifier(attr.get().value(), uuid, m.amount(), operation(m.operation()));
            }
            Registry.register(Registries.STATUS_EFFECT, id, effect);
        }

        for (YogContent.Potion p : content.potions) {
            List<StatusEffectInstance> effects = new ArrayList<>();
            for (YogContent.PotionEffect pe : p.effects()) {
                var effect = java.util.Optional.ofNullable(Registries.STATUS_EFFECT.get(new Identifier(pe.effect())));
                if (effect.isEmpty()) {
                    System.err.println("[yog] potion " + p.id() + ": no effect " + pe.effect());
                    continue;
                }
                effects.add(new StatusEffectInstance(effect.get(), pe.duration(), pe.amplifier()));
            }
            Identifier id = potionId(p.id());
            if (!p.id().startsWith("yog:potion/") && !Registries.POTION.containsId(id)) {
                Registry.register(Registries.POTION, id, new Potion(p.name(), effects.toArray(new StatusEffectInstance[0])));
            }
        }
        for (YogContent.Potion p : content.potions) {
            Potion to = Registries.POTION.get(potionId(p.id()));
            for (YogContent.Brew b : p.brewed()) {
                Identifier from = potionId(b.from());
                Identifier with = new Identifier(b.with());
                if (!Registries.POTION.containsId(from) || !Registries.ITEM.containsId(with)) {
                    System.err.println("[yog] potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                    continue;
                }
                FabricBrewingRecipeRegistry.registerPotionRecipe(Registries.POTION.get(from),
                        net.minecraft.recipe.Ingredient.ofItems(Registries.ITEM.get(with)), to);
            }
        }
    }

    private static Identifier potionId(String id) {
        if (id.startsWith("yog:potion/")) return new Identifier("minecraft", id.substring("yog:potion/".length()));
        return id.startsWith("yog:") ? new Identifier("minecraft", id.substring(4)) : new Identifier(id);
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
            case "multiply_base" -> EntityAttributeModifier.Operation.MULTIPLY_BASE;
            case "multiply_total" -> EntityAttributeModifier.Operation.MULTIPLY_TOTAL;
            default -> EntityAttributeModifier.Operation.ADDITION;
        };
    }

    /** A mob effect of a mod: what it does is its attributes' modifiers. */
    private static final class Effect extends StatusEffect {
        Effect(StatusEffectCategory category, int color) {
            super(category, color);
        }
    }
}
