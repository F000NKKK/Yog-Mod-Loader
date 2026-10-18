package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * A model is animated right before it's drawn (in 26.x drawing comes
 * after the entity was submitted): yog-body's part changes go on then.
 */
@Mixin(ModelFeatureRenderer.class)
public abstract class ModelPoseMixin {
    @Redirect(method = "prepareModel", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/model/Model;setupAnim(Ljava/lang/Object;)V"))
    private <S> void yog$posed(Model<S> model, S state) {
        model.setupAnim(state);
        YogBody.posed(model, state);
    }
}
