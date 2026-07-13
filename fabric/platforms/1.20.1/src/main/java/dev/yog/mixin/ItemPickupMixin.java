package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.entity.ItemEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ItemEntity.class)
public class ItemPickupMixin {

    @Inject(method = "onPlayerCollision", at = @At("HEAD"), cancellable = true)
    private void yog$onItemPickup(PlayerEntity player, CallbackInfo ci) {
        if (!(player instanceof ServerPlayerEntity sp)) return;
        ItemEntity self = (ItemEntity)(Object)this;
        ItemStack stack = self.getStack();
        String itemId = Registries.ITEM.getId(stack.getItem()).toString();
        int count = stack.getCount();
        String entityUuid = self.getUuidAsString();
        String dim = self.getWorld().getRegistryKey().getValue().toString();
        boolean allow = NativeBridge.nativeOnItemPickupPre(
                sp.getName().getString(), sp.getUuidAsString(),
                itemId, count, entityUuid, dim);
        if (!allow) {
            ci.cancel();
            return;
        }
        NativeBridge.nativeOnItemPickup(
                sp.getName().getString(), sp.getUuidAsString(),
                itemId, count, entityUuid, dim);
    }
}
