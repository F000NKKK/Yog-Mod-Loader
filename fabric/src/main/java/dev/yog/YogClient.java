package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.util.Identifier;

/** Client-side entry point: wires client packet receivers (server -> client). */
public class YogClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        NativeBridge.ensureLoaded();

        String channels = NativeBridge.nativeClientPacketChannels();
        if (channels == null) {
            return;
        }
        for (String channel : channels.split("\n")) {
            if (channel.isBlank()) {
                continue;
            }
            Identifier id = Identifier.tryParse(channel);
            if (id == null) {
                continue;
            }
            ClientPlayNetworking.registerGlobalReceiver(id, (client, handler, buf, sender) -> {
                byte[] data = new byte[buf.readableBytes()];
                buf.readBytes(data);
                client.execute(() -> NativeBridge.nativeOnClientPacket(channel, data));
            });
        }
    }
}
