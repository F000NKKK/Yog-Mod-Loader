package dev.yog.mixin;

import net.minecraft.core.Registry;
import dev.yog.bus.YogEvents;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.ResultSlot;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ResultSlot.class)
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
public class CraftingMixin {

    @Inject(
        method = "onTake(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/item/ItemStack;)V",
        at = @At("HEAD")
    )
    private void yog$onCraft(Player player, ItemStack stack, CallbackInfo ci) {
        if (!(player instanceof ServerPlayer sp)) return;
        String itemId = Registry.ITEM.getKey(stack.getItem()).toString();
        new YogEvents.Player.Action.Craft(sp.getName().getString(), sp.getStringUUID(), itemId, stack.getCount(), sp.getLevel().dimension().location().toString()).emit();
    }
}
