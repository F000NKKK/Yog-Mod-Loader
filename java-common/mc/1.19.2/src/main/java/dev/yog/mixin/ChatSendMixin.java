package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** What the player sends to the chat goes past the mods first; one may keep it from the server. */
@Mixin(LocalPlayer.class)
public abstract class ChatSendMixin {
    @Inject(method = "chatSigned", at = @At("HEAD"), cancellable = true)
    private void yog$onChat(String text, Component preview, CallbackInfo ci) {
        if (!new YogEvents.Client.Chat.Send(text).emitPre()) ci.cancel();
    }
}
