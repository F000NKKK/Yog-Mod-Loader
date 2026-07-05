package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.context.BlockPlaceContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fires {@code nativeOnPlaceBlock} (Post phase) after a block is successfully placed.
 * Pre phase is handled in YogHost via RightClickBlock, which can cancel placement.
 */
@Mixin(BlockItem.class)
public class BlockItemPlaceMixin {

    @Inject(method = "place", at = @At("RETURN"))
    private void yog$onBlockPlaced(BlockPlaceContext context, CallbackInfoReturnable<InteractionResult> cir) {
        InteractionResult result = cir.getReturnValue();
        if (result != InteractionResult.SUCCESS && result != InteractionResult.CONSUME) return;
        if (context.getLevel().isClientSide()) return;
        if (!(context.getPlayer() instanceof ServerPlayer sp)) return;

        BlockPos pos = context.getClickedPos();
        String blockId = BuiltInRegistries.BLOCK.getKey(((BlockItem) (Object) this).getBlock()).toString();
        NativeBridge.nativeOnPlaceBlock(
                sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
    }
}
