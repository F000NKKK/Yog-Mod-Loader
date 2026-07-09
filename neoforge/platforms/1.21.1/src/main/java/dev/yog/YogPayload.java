package dev.yog;

import java.nio.charset.StandardCharsets;

import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceLocation;

/**
 * Single bidirectional payload multiplexing every one of Yog's dynamic,
 * mod-declared packet channels. NeoForge's typed networking
 * (`RegisterPayloadHandlersEvent`/`PayloadRegistrar`) wants every payload
 * `Type` declared once, up front — but Yog mods register channel *names* at
 * runtime (`on_packet`, `on_client_packet`, `send_to_player`,
 * `send_to_server`), so this wraps every message as one "yog:bridge" type
 * carrying `[u16 name length][name utf8][raw data]`, demultiplexed by name
 * in `YogHost`'s single payload handler.
 */
public record YogPayload(String channelName, byte[] data) implements CustomPacketPayload {
    public static final Type<YogPayload> TYPE =
            new Type<>(ResourceLocation.fromNamespaceAndPath("yog", "bridge"));

    public static final StreamCodec<FriendlyByteBuf, YogPayload> CODEC = StreamCodec.of(
            (buf, payload) -> {
                byte[] nameBytes = payload.channelName.getBytes(StandardCharsets.UTF_8);
                buf.writeShort(nameBytes.length);
                buf.writeBytes(nameBytes);
                buf.writeBytes(payload.data);
            },
            buf -> {
                int nameLen = buf.readUnsignedShort();
                byte[] nameBytes = new byte[nameLen];
                buf.readBytes(nameBytes);
                String channelName = new String(nameBytes, StandardCharsets.UTF_8);
                byte[] data = new byte[buf.readableBytes()];
                buf.readBytes(data);
                return new YogPayload(channelName, data);
            });

    @Override
    public Type<? extends CustomPacketPayload> type() {
        return TYPE;
    }
}
