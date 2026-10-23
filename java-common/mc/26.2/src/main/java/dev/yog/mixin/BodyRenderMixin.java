package dev.yog.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.yog.body.YogBody;
import net.minecraft.client.model.EntityModel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** yog-body's hooks in extracting and drawing a living entity (see {@link YogBody}). */
@Mixin(LivingEntityRenderer.class)
public abstract class BodyRenderMixin<T extends LivingEntity, S extends LivingEntityRenderState, M extends EntityModel<? super S>> {
    @Shadow protected M model;

    private static final String EXTRACT =
            "extractRenderState(Lnet/minecraft/world/entity/LivingEntity;Lnet/minecraft/client/renderer/entity/state/LivingEntityRenderState;F)V";
    private static final String SUBMIT =
            "submit(Lnet/minecraft/client/renderer/entity/state/LivingEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;"
            + "Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V";

    @Inject(method = EXTRACT, at = @At("TAIL"))
    private void yog$extract(LivingEntity e, LivingEntityRenderState state, float partial, CallbackInfo ci) {
        YogBody.extract((LivingEntityRenderer<?, ?, ?>) (Object) this, e, state, partial);
    }

    @Inject(method = SUBMIT, at = @At("HEAD"), cancellable = true)
    private void yog$begin(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera, CallbackInfo ci) {
        if (YogBody.begin(state, pose, out, camera)) ci.cancel();
    }

    /** This scale comes right after {@code setupRotations}. */
    @Inject(method = SUBMIT, at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;scale(FFF)V", ordinal = 1))
    private void yog$rotated(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera, CallbackInfo ci) {
        YogBody.rotated(state, pose, out);
    }

    /** The model animated for its layers: the mods' part changes. */
    @Inject(method = SUBMIT, at = @At(value = "INVOKE",
            target = "Lnet/minecraft/client/model/EntityModel;setupAnim(Ljava/lang/Object;)V", shift = At.Shift.AFTER))
    private void yog$posed(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera, CallbackInfo ci) {
        YogBody.posed(model, state);
    }

    @Inject(method = SUBMIT, at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;popPose()V"))
    private void yog$drawn(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera, CallbackInfo ci) {
        YogBody.drawn(model, state, pose, out);
    }
}
