package dev.yog.server;

import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.ItemTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.level.block.Block;

/** What digging speed is made of, this version's way ({@link YogDigSpeeds}). */
final class YogDigging {
    private YogDigging() {}

    /** Tool materials, cheapest first. */
    private static final List<TagKey<Item>> MATERIALS = List.of(ItemTags.WOODEN_TOOL_MATERIALS, ItemTags.STONE_TOOL_MATERIALS,
            ItemTags.IRON_TOOL_MATERIALS, ItemTags.GOLD_TOOL_MATERIALS, ItemTags.DIAMOND_TOOL_MATERIALS, ItemTags.NETHERITE_TOOL_MATERIALS);

    static Block block(String id) {
        Identifier r = Identifier.tryParse(id);
        return r == null ? null : BuiltInRegistries.BLOCK.getOptional(r).orElse(null);
    }

    /** Efficiency's enchantment level on {@code stack}. */
    static int efficiency(Player p, ItemStack stack) {
        return EnchantmentHelper.getItemEnchantmentLevel(
                p.registryAccess().lookupOrThrow(Registries.ENCHANTMENT).getOrThrow(Enchantments.EFFICIENCY), stack);
    }

    /** Haste's amplifier, -1 without it. */
    static int haste(Player p) {
        MobEffectInstance e = p.getEffect(MobEffects.HASTE);
        return e == null ? -1 : e.getAmplifier();
    }

    /** Mining fatigue's amplifier, -1 without it. */
    static int fatigue(Player p) {
        MobEffectInstance e = p.getEffect(MobEffects.MINING_FATIGUE);
        return e == null ? -1 : e.getAmplifier();
    }

    static boolean sword(ItemStack stack) {
        return stack.is(ItemTags.SWORDS);
    }

    /** The tool's material, cheapest first; -1 for none. */
    static int material(ItemStack stack) {
        for (int i = 0; i < MATERIALS.size(); i++) {
            if (stack.is(MATERIALS.get(i))) return i;
        }
        return -1;
    }
}
