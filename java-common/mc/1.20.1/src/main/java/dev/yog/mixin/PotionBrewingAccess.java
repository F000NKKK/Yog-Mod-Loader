package dev.yog.mixin;

import net.minecraft.world.item.Item;
import net.minecraft.world.item.alchemy.Potion;
import net.minecraft.world.item.alchemy.PotionBrewing;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The game's own brewing mixes (1.20.1 keeps them private): the mods' go in beside them, on every loader. */
@Mixin(PotionBrewing.class)
public interface PotionBrewingAccess {
    @Invoker("addMix")
    static void yog$addMix(Potion from, Item with, Potion to) {
        throw new AssertionError();
    }
}
