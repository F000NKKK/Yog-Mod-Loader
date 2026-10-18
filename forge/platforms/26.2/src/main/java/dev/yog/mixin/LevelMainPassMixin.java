package dev.yog.mixin;

import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.resource.ResourceHandle;
import dev.yog.YogClientEvents;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.chunk.ChunkSectionsToRender;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.util.profiling.ProfilerFiller;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The end of the level's main pass: the mods draw in the world (Forge has
 * no event there; Fabric's {@code LevelRenderEvents.END_MAIN} is this spot).
 */
@Mixin(LevelRenderer.class)
public abstract class LevelMainPassMixin {
    @Inject(method = "lambda$addMainPass$0", at = @At("TAIL"))
    private void yog$mainPass(GpuBufferSlice fog, LevelRenderState state, ProfilerFiller profiler, ChunkSectionsToRender sections,
                              ResourceHandle<?> main, FeatureRenderDispatcher.PreparedFrame frame, ResourceHandle<?> translucent,
                              ResourceHandle<?> itemEntity, ResourceHandle<?> particles, ResourceHandle<?> weather, CallbackInfo ci) {
        var camera = state.cameraRenderState;
        YogClientEvents.world(camera.viewRotationMatrix, camera.pos, Minecraft.getInstance().getDeltaTracker().getGameTimeDeltaPartialTick(false));
    }
}
