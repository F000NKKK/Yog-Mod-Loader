package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.MenuProvider;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.OptionalInt;

@Mixin(ServerPlayer.class)
public class ContainerOpenMixin {

    @Inject(
        method = "openMenu(Lnet/minecraft/world/MenuProvider;)Ljava/util/OptionalInt;",
        at = @At("HEAD"),
        cancellable = true
    )
    private void yog$onContainerOpenPre(MenuProvider factory,
                                         CallbackInfoReturnable<OptionalInt> cir) {
        ServerPlayer sp = (ServerPlayer)(Object)this;
        boolean allow = NativeBridge.nativeOnContainerOpenPre(
                sp.getName().getString(), sp.getStringUUID());
        if (!allow) cir.setReturnValue(OptionalInt.empty());
    }

    @Inject(
        method = "openMenu(Lnet/minecraft/world/MenuProvider;)Ljava/util/OptionalInt;",
        at = @At("RETURN")
    )
    private void yog$onContainerOpen(MenuProvider factory,
                                      CallbackInfoReturnable<OptionalInt> cir) {
        if (!cir.getReturnValue().isPresent()) return;
        ServerPlayer sp = (ServerPlayer)(Object)this;
        ResourceLocation typeId = BuiltInRegistries.MENU.getKey(sp.containerMenu.getType());
        String containerType = typeId != null ? typeId.toString() : "";
        NativeBridge.nativeOnContainerOpen(
                sp.getName().getString(), sp.getStringUUID(), containerType);
    }
}
