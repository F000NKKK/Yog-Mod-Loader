package dev.yog.server;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.SwordItem;
import net.minecraft.world.item.TieredItem;
import net.minecraft.world.item.Tiers;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.level.block.Block;

/** What digging speed is made of, this version's way ({@link YogDigSpeeds}). */
final class YogDigging {
    private YogDigging() {}

    static Block block(String id) {
        ResourceLocation r = ResourceLocation.tryParse(id);
        return r == null ? null : BuiltInRegistries.BLOCK.getOptional(r).orElse(null);
    }

    /** Efficiency's enchantment level on {@code stack}. */
    static int efficiency(Player p, ItemStack stack) {
        return EnchantmentHelper.getItemEnchantmentLevel(p.registryAccess().registryOrThrow(Registries.ENCHANTMENT).getHolderOrThrow(Enchantments.EFFICIENCY), stack);
    }

    /** Haste's amplifier, -1 without it. */
    static int haste(Player p) {
        MobEffectInstance e = p.getEffect(MobEffects.DIG_SPEED);
        return e == null ? -1 : e.getAmplifier();
    }

    /** Mining fatigue's amplifier, -1 without it. */
    static int fatigue(Player p) {
        MobEffectInstance e = p.getEffect(MobEffects.DIG_SLOWDOWN);
        return e == null ? -1 : e.getAmplifier();
    }

    static boolean sword(ItemStack stack) {
        return stack.getItem() instanceof SwordItem;
    }

    /** The tool's material, cheapest first; -1 for none. */
    static int material(ItemStack stack) {
        return stack.getItem() instanceof TieredItem t && t.getTier() instanceof Tiers tier ? tier.ordinal() : -1;
    }
}
