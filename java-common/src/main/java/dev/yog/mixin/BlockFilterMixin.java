package dev.yog.mixin;

import dev.yog.render.YogBlockFilter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.state.BlockBehaviour;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A block {@link YogBlockFilter} hides: drawn as nothing and hiding nothing
 * behind it — what the game's and Sodium's chunk building both ask of a
 * block state. The filter's off check comes first: these run for every
 * block a chunk's drawing goes through.
 */
@Mixin(BlockBehaviour.BlockStateBase.class)
public abstract class BlockFilterMixin {
    @Shadow
    public abstract Block getBlock();

    @Inject(method = "getRenderShape", at = @At("HEAD"), cancellable = true)
    private void yog$shape(CallbackInfoReturnable<RenderShape> cir) {
        if (YogBlockFilter.hidden(getBlock())) cir.setReturnValue(RenderShape.INVISIBLE);
    }

    @Inject(method = "canOcclude", at = @At("HEAD"), cancellable = true)
    private void yog$occludes(CallbackInfoReturnable<Boolean> cir) {
        if (YogBlockFilter.hidden(getBlock())) cir.setReturnValue(false);
    }
}
