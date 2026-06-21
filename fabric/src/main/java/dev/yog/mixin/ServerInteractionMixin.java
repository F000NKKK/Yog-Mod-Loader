package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.block.BlockState;
import net.minecraft.registry.Registries;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.network.ServerPlayerInteractionManager;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.util.math.BlockPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Hooks block breaking on the server and forwards it to the Rust runtime.
 *
 * NOTE: the Yarn names below target MC 1.20.1 and MUST be confirmed against the
 * exact Yarn build pinned in gradle.properties — Mixin shadow field/method
 * names and signatures can shift between mapping builds. In particular verify:
 *   - {@code ServerPlayerInteractionManager} fields {@code player}, {@code world}
 *   - the {@code tryBreakBlock(BlockPos)} method name/signature
 */
@Mixin(ServerPlayerInteractionManager.class)
public class ServerInteractionMixin {
    @Shadow
    protected ServerPlayerEntity player;

    @Shadow
    protected ServerWorld world;

    @Inject(method = "tryBreakBlock", at = @At("HEAD"))
    private void yog$onTryBreakBlock(BlockPos pos, CallbackInfoReturnable<Boolean> cir) {
        BlockState state = world.getBlockState(pos);
        String blockId = Registries.BLOCK.getId(state.getBlock()).toString();
        String name = player.getName().getString();
        NativeBridge.nativeOnBlockBreak(name, blockId, pos.getX(), pos.getY(), pos.getZ());
    }
}
