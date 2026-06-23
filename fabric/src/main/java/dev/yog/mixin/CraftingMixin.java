package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.screen.slot.CraftingResultSlot;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(CraftingResultSlot.class)
public class CraftingMixin {

    @Inject(
        method = "onTakeItem(Lnet/minecraft/entity/player/PlayerEntity;Lnet/minecraft/item/ItemStack;)V",
        at = @At("HEAD")
    )
    private void yog$onCraft(PlayerEntity player, ItemStack stack, CallbackInfo ci) {
        if (stack.isEmpty()) return;
        if (!(player instanceof ServerPlayerEntity sp)) return;
        String itemId = Registries.ITEM.getId(stack.getItem()).toString();
        NativeBridge.nativeOnItemCraft(
            sp.getName().getString(), sp.getUuidAsString(),
            itemId, stack.getCount());
    }
}
