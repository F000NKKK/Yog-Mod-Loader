package dev.yog.mixin;

import net.minecraft.core.Registry;
import dev.yog.bus.YogEvents;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.MenuProvider;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.OptionalInt;

@Mixin(ServerPlayer.class)
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
public class ContainerOpenMixin {

    @Inject(
        method = "openMenu(Lnet/minecraft/world/MenuProvider;)Ljava/util/OptionalInt;",
        at = @At("HEAD"),
        cancellable = true
    )
    private void yog$onContainerOpenPre(MenuProvider factory,
                                         CallbackInfoReturnable<OptionalInt> cir) {
        ServerPlayer sp = (ServerPlayer)(Object)this;
        boolean allow = new YogEvents.Container.Open(sp.getName().getString(), sp.getStringUUID(), "", sp.getLevel().dimension().location().toString()).emitPre();
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
        ResourceLocation typeId = Registry.MENU.getKey(sp.containerMenu.getType());
        String containerType = typeId != null ? typeId.toString() : "";
        new YogEvents.Container.Open(sp.getName().getString(), sp.getStringUUID(), containerType, sp.getLevel().dimension().location().toString()).emit();
    }
}
