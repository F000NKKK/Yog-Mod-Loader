package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.advancement.Advancement;
import net.minecraft.advancement.AdvancementProgress;
import net.minecraft.advancement.PlayerAdvancementTracker;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(PlayerAdvancementTracker.class)
public abstract class AdvancementMixin {

    @Shadow private ServerPlayerEntity owner;

    @Shadow public abstract AdvancementProgress getProgress(Advancement advancement);

    // Yarn: grantCriterion(Advancement, String) -> boolean (method_12878)
    @Inject(
        method = "grantCriterion(Lnet/minecraft/advancement/Advancement;Ljava/lang/String;)Z",
        at = @At("RETURN")
    )
    private void yog$onCriterionGrant(
            Advancement advancement, String criterionName,
            CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValue()) return;
        if (!getProgress(advancement).isDone()) return;
        if (owner == null || advancement.getId() == null) return;
        NativeBridge.nativeOnAdvancement(
                owner.getName().getString(),
                owner.getUuidAsString(),
                advancement.getId().toString());
    }
}
