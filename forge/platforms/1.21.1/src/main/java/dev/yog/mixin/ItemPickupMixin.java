package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ItemEntity.class)
public class ItemPickupMixin {

    @Inject(method = "playerTouch", at = @At("HEAD"), cancellable = true)
    private void yog$onItemPickup(Player player, CallbackInfo ci) {
        if (!(player instanceof ServerPlayer sp)) return;
        ItemEntity self = (ItemEntity)(Object)this;
        ItemStack stack = self.getItem();
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        int count = stack.getCount();
        String entityUuid = self.getStringUUID();
        boolean allow = NativeBridge.nativeOnItemPickupPre(
                sp.getName().getString(), sp.getStringUUID(),
                itemId, count, entityUuid);
        if (!allow) {
            ci.cancel();
            return;
        }
        NativeBridge.nativeOnItemPickup(
                sp.getName().getString(), sp.getStringUUID(),
                itemId, count, entityUuid);
    }
}
