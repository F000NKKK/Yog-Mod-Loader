package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.advancement.AdvancementEntry;
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

    @Shadow public abstract AdvancementProgress getProgress(AdvancementEntry advancement);

    // Yarn: grantCriterion(AdvancementEntry, String) -> boolean
    @Inject(
        method = "grantCriterion(Lnet/minecraft/advancement/AdvancementEntry;Ljava/lang/String;)Z",
        at = @At("RETURN")
    )
    private void yog$onCriterionGrant(
            AdvancementEntry advancement, String criterionName,
            CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValue()) return;
        if (!getProgress(advancement).isDone()) return;
        if (owner == null || advancement.id() == null) return;
        NativeBridge.nativeOnAdvancement(
                owner.getName().getString(),
                owner.getUuidAsString(),
                advancement.id().toString(),
                owner.getWorld().getRegistryKey().getValue().toString());
    }
}
