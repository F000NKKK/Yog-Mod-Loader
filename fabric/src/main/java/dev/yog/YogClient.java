package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.networking.v1.PacketByteBufs;
import net.minecraft.network.PacketByteBuf;
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

    /** Send a raw-byte packet to the server (client -> server). */
    public static boolean sendToServer(String channel, byte[] data) {
        Identifier id = Identifier.tryParse(channel);
        if (id == null) {
            return false;
        }
        try {
            PacketByteBuf buf = PacketByteBufs.create();
            buf.writeBytes(data);
            ClientPlayNetworking.send(id, buf);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
