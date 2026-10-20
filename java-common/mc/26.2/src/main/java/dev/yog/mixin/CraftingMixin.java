package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.ResultSlot;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ResultSlot.class)
public class CraftingMixin {

    @Inject(
        method = "onTake",
        at = @At("HEAD")
    )
    private void yog$onCraft(Player player, ItemStack stack, CallbackInfo ci) {
        if (!(player instanceof ServerPlayer sp)) return;
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        new YogEvents.Player.Action.Craft(sp.getName().getString(), sp.getStringUUID(), itemId, stack.getCount(), sp.level().dimension().identifier().toString()).emit();
    }
}
