package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.item.BlockItem;
import net.minecraft.item.ItemPlacementContext;
import net.minecraft.registry.Registries;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.util.ActionResult;
import net.minecraft.util.math.BlockPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fires {@code nativeOnPlaceBlock} (Post phase) after a block is successfully placed.
 * Pre phase is handled in YogHost via UseBlockCallback, which can cancel placement.
 */
@Mixin(BlockItem.class)
public class BlockItemPlaceMixin {

    @Inject(method = "place", at = @At("RETURN"))
    private void yog$onBlockPlaced(ItemPlacementContext context, CallbackInfoReturnable<ActionResult> cir) {
        ActionResult result = cir.getReturnValue();
        if (result != ActionResult.SUCCESS && result != ActionResult.CONSUME) return;
        if (context.getWorld().isClient()) return;
        if (!(context.getPlayer() instanceof ServerPlayerEntity sp)) return;

        BlockPos pos = context.getBlockPos();
        String blockId = Registries.BLOCK.getId(((BlockItem) (Object) this).getBlock()).toString();
        NativeBridge.nativeOnPlaceBlock(
                sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
    }
}
