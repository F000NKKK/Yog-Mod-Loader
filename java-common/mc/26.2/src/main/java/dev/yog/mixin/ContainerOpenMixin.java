package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.OptionalInt;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.MenuProvider;

@Mixin(ServerPlayer.class)
public class ContainerOpenMixin {

    @Inject(
        method = "openMenu",
        at = @At("HEAD"),
        cancellable = true
    )
    private void yog$onContainerOpenPre(MenuProvider factory,
                                         CallbackInfoReturnable<OptionalInt> cir) {
        ServerPlayer sp = (ServerPlayer)(Object)this;
        boolean allow = new YogEvents.Container.Open(sp.getName().getString(), sp.getStringUUID(), "", sp.level().dimension().identifier().toString()).emitPre();
        if (!allow) cir.setReturnValue(OptionalInt.empty());
    }

    @Inject(
        method = "openMenu",
        at = @At("RETURN")
    )
    private void yog$onContainerOpen(MenuProvider factory,
                                      CallbackInfoReturnable<OptionalInt> cir) {
        if (!cir.getReturnValue().isPresent()) return;
        ServerPlayer sp = (ServerPlayer)(Object)this;
        Identifier typeId = BuiltInRegistries.MENU.getKey(sp.containerMenu.getType());
        String containerType = typeId != null ? typeId.toString() : "";
        new YogEvents.Container.Open(sp.getName().getString(), sp.getStringUUID(), containerType, sp.level().dimension().identifier().toString()).emit();
    }
}
