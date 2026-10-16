package dev.yog;

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
import net.minecraftforge.common.brewing.BrewingRecipeRegistry;
import net.minecraftforge.common.brewing.IBrewingRecipe;
import net.minecraftforge.event.entity.EntityAttributeModificationEvent;
import net.minecraftforge.eventbus.api.IEventBus;
import net.minecraftforge.fml.event.lifecycle.FMLCommonSetupEvent;
import net.minecraftforge.registries.RegisterEvent;

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

    /** Listens on the mod bus (registries, the player's attributes, setup). */
    static void listen(IEventBus modBus) {
        modBus.addListener(YogEffects::onRegister);
        modBus.addListener(YogEffects::onPlayerAttributes);
        modBus.addListener(YogEffects::onSetup);
    }

    private static YogContent content() {
        return YogContent.cached(NativeBridge::nativeContent);
    }

    private static ResourceLocation id(String id) {
        return new ResourceLocation(id);
    }

    private static ResourceLocation potionId(String id) {
        return id.startsWith("yog:") ? new ResourceLocation("minecraft", id.substring(4)) : id(id);
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
                    MobEffect effect = BuiltInRegistries.MOB_EFFECT.get(id(pe.effect()));
                    if (effect == null) {
                        System.err.println("[yog] potion " + p.id() + ": no effect " + pe.effect());
                        continue;
                    }
                    effects.add(new MobEffectInstance(effect, pe.duration(), pe.amplifier()));
                }
                Potion potion = new Potion(p.name(), effects.toArray(new MobEffectInstance[0]));
                if (!p.id().startsWith("yog:potion/")) event.register(Registries.POTION, id(p.id()), () -> potion);
            }
        } else if (event.getRegistryKey().equals(Registries.ATTRIBUTE)) {
            for (YogContent.Attribute a : content().attributes) {
                ResourceLocation rl = id(a.id());
                Attribute attr = new RangedAttribute("attribute.name." + rl.getNamespace() + "." + rl.getPath(),
                        a.defaultValue(), a.min(), a.max()).setSyncable(true);
                ATTRIBUTES.put(a.id(), attr);
                event.register(Registries.ATTRIBUTE, rl, () -> attr);
            }
        }
    }

    private static void onPlayerAttributes(EntityAttributeModificationEvent event) {
        for (Attribute attr : ATTRIBUTES.values()) {
            event.add(EntityType.PLAYER, attr);
        }
    }

    private static void onSetup(FMLCommonSetupEvent event) {
        event.enqueueWork(() -> {
            for (YogContent.Effect e : content().effects) {
                MobEffect effect = EFFECTS.get(e.id());
                if (effect == null) continue;
                for (YogContent.Modifier m : e.modifiers()) {
                    Attribute attr = BuiltInRegistries.ATTRIBUTE.get(id(m.attribute()));
                    if (attr == null) {
                        System.err.println("[yog] effect " + e.id() + ": no attribute " + m.attribute());
                        continue;
                    }
                    // A modifier's UUID: the same for an effect and attribute every run.
                    String uuid = java.util.UUID.nameUUIDFromBytes((e.id() + "/" + m.attribute())
                            .getBytes(java.nio.charset.StandardCharsets.UTF_8)).toString();
                    effect.addAttributeModifier(attr, uuid, m.amount(), operation(m.operation()));
                }
            }
            for (YogContent.Potion p : content().potions) {
                Potion to = BuiltInRegistries.POTION.get(potionId(p.id()));
                for (YogContent.Brew b : p.brewed()) {
                    ResourceLocation from = potionId(b.from()), with = id(b.with());
                    if (!BuiltInRegistries.POTION.containsKey(from) || !BuiltInRegistries.ITEM.containsKey(with)) {
                        System.err.println("[yog] potion " + p.id() + ": can't brew from " + b.from() + " with " + b.with());
                        continue;
                    }
                    BrewingRecipeRegistry.addRecipe(new Brewing(BuiltInRegistries.POTION.get(from), BuiltInRegistries.ITEM.get(with), to));
                }
            }
        });
    }

    /** A brewing stand recipe: a potion of `from` (any bottle) with `with` makes one of `to`. */
    private record Brewing(Potion from, net.minecraft.world.item.Item with, Potion to) implements IBrewingRecipe {
        @Override
        public boolean isInput(net.minecraft.world.item.ItemStack stack) {
            return net.minecraft.world.item.alchemy.PotionUtils.getPotion(stack) == from;
        }

        @Override
        public boolean isIngredient(net.minecraft.world.item.ItemStack stack) {
            return stack.is(with);
        }

        @Override
        public net.minecraft.world.item.ItemStack getOutput(net.minecraft.world.item.ItemStack input, net.minecraft.world.item.ItemStack ingredient) {
            if (!isInput(input) || !isIngredient(ingredient)) return net.minecraft.world.item.ItemStack.EMPTY;
            return net.minecraft.world.item.alchemy.PotionUtils.setPotion(new net.minecraft.world.item.ItemStack(input.getItem()), to);
        }
    }

    private static MobEffectCategory category(String c) {
        return switch (c) {
            case "beneficial" -> MobEffectCategory.BENEFICIAL;
            case "harmful" -> MobEffectCategory.HARMFUL;
            default -> MobEffectCategory.NEUTRAL;
        };
    }

    private static AttributeModifier.Operation operation(String o) {
        return switch (o) {
            case "multiply_base" -> AttributeModifier.Operation.MULTIPLY_BASE;
            case "multiply_total" -> AttributeModifier.Operation.MULTIPLY_TOTAL;
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
