package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.client.render.entity.model.EntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** yog-body's hooks in drawing a living entity (see {@link YogBody}). */
@Mixin(LivingEntityRenderer.class)
public abstract class BodyRenderMixin<T extends LivingEntity, M extends EntityModel<T>> {
    @Shadow protected M model;

    private static final String RENDER =
            "render(Lnet/minecraft/entity/LivingEntity;FFLnet/minecraft/client/util/math/MatrixStack;Lnet/minecraft/client/render/VertexConsumerProvider;I)V";

    @Inject(method = RENDER, at = @At("HEAD"), cancellable = true)
    private void yog$begin(LivingEntity e, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light, CallbackInfo ci) {
        if (YogBody.begin((LivingEntityRenderer<?, ?>) (Object) this, e, yaw, partial, pose, buffers, light)) ci.cancel();
    }

    /** This scale comes right after {@code setupTransforms}. */
    @Inject(method = RENDER, at = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/math/MatrixStack;scale(FFF)V", ordinal = 1))
    private void yog$rotated(LivingEntity e, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light, CallbackInfo ci) {
        YogBody.rotated(pose, buffers, light);
    }

    @Inject(method = RENDER, at = @At(value = "INVOKE",
            target = "Lnet/minecraft/client/render/entity/model/EntityModel;setAngles(Lnet/minecraft/entity/Entity;FFFFF)V", shift = At.Shift.AFTER))
    private void yog$posed(LivingEntity e, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light, CallbackInfo ci) {
        YogBody.posed(model);
    }

    @Inject(method = RENDER, at = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/math/MatrixStack;pop()V"))
    private void yog$drawn(LivingEntity e, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light, CallbackInfo ci) {
        YogBody.drawn(model, pose, buffers, light);
    }

    @Inject(method = RENDER, at = @At("RETURN"))
    private void yog$end(LivingEntity e, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light, CallbackInfo ci) {
        YogBody.end();
    }
}
