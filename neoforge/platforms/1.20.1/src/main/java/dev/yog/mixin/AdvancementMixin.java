package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.advancements.Advancement;
import net.minecraft.advancements.AdvancementProgress;
import net.minecraft.server.PlayerAdvancements;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(PlayerAdvancements.class)
public abstract class AdvancementMixin {

    @Shadow private ServerPlayer player;

    @Shadow public abstract AdvancementProgress getOrStartProgress(Advancement advancement);

    @Inject(
        method = "award(Lnet/minecraft/advancements/Advancement;Ljava/lang/String;)Z",
        at = @At("RETURN")
    )
    private void yog$onCriterionGrant(
            Advancement advancement, String criterionName,
            CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValue()) return;
        if (!getOrStartProgress(advancement).isDone()) return;
        if (player == null || advancement.getId() == null) return;
        NativeBridge.nativeOnAdvancement(
                player.getName().getString(),
                player.getStringUUID(),
                advancement.getId().toString());
    }
}
