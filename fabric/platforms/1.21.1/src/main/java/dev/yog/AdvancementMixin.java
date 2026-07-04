package dev.yog.mixin;

import net.minecraft.advancement.AdvancementEntry;
import net.minecraft.advancement.PlayerAdvancementTracker;
import net.minecraft.server.network.ServerPlayerEntity;
import dev.yog.NativeBridge;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(PlayerAdvancementTracker.class)
public abstract class AdvancementMixin {

    @Shadow
    private ServerPlayerEntity owner;

    @Shadow
    public abstract boolean grantCriterion(AdvancementEntry advancement, String criterion);

    @Inject(
        method = "grantCriterion(Lnet/minecraft/advancement/AdvancementEntry;Ljava/lang/String;)Z",
        at = @At("RETURN")
    )
    private void onAdvancementGranted(AdvancementEntry advancement, String criterion,
                                       CallbackInfoReturnable<Boolean> cir) {
        if (cir.getReturnValueZ()) {
            NativeBridge.nativeOnAdvancement(
                owner.getName().getString(),
                owner.getUuidAsString(),
                advancement.id().toString()
            );
        }
    }
}