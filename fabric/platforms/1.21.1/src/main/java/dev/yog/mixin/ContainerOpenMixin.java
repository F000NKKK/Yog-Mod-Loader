package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.registry.Registries;
import net.minecraft.screen.NamedScreenHandlerFactory;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.util.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.OptionalInt;

@Mixin(ServerPlayerEntity.class)
public class ContainerOpenMixin {

    @Inject(
        method = "openHandledScreen(Lnet/minecraft/screen/NamedScreenHandlerFactory;)Ljava/util/OptionalInt;",
        at = @At("HEAD"),
        cancellable = true
    )
    private void yog$onContainerOpenPre(NamedScreenHandlerFactory factory,
                                         CallbackInfoReturnable<OptionalInt> cir) {
        ServerPlayerEntity sp = (ServerPlayerEntity)(Object)this;
        boolean allow = NativeBridge.nativeOnContainerOpenPre(
                sp.getName().getString(), sp.getUuidAsString(),
                sp.getWorld().getRegistryKey().getValue().toString());
        if (!allow) cir.setReturnValue(OptionalInt.empty());
    }

    @Inject(
        method = "openHandledScreen(Lnet/minecraft/screen/NamedScreenHandlerFactory;)Ljava/util/OptionalInt;",
        at = @At("RETURN")
    )
    private void yog$onContainerOpen(NamedScreenHandlerFactory factory,
                                      CallbackInfoReturnable<OptionalInt> cir) {
        if (!cir.getReturnValue().isPresent()) return;
        ServerPlayerEntity sp = (ServerPlayerEntity)(Object)this;
        Identifier typeId = Registries.SCREEN_HANDLER.getId(sp.currentScreenHandler.getType());
        String containerType = typeId != null ? typeId.toString() : "";
        NativeBridge.nativeOnContainerOpen(
                sp.getName().getString(), sp.getUuidAsString(), containerType,
                sp.getWorld().getRegistryKey().getValue().toString());
    }
}
