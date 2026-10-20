package dev.yog;

import java.nio.charset.StandardCharsets;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceLocation;

/**
 * Every one of the mods' packet channels, as one payload type. The game's
 * networking wants each payload type declared up front, but mods name
 * their channels at run time (`on_packet`, `send_to_player`, …): so a
 * message is one {@code yog:bridge} payload carrying
 * {@code [u16 name length][name utf8][raw data]}, handed on by its name.
 * The same on every loader; each registers the type its own way.
 */
public record YogPayload(String channel, byte[] data) implements CustomPacketPayload {
    public static final Type<YogPayload> TYPE = new Type<>(ResourceLocation.fromNamespaceAndPath(YogNames.Ns.YOG, "bridge"));

    public static final StreamCodec<FriendlyByteBuf, YogPayload> CODEC = StreamCodec.of(
            (buf, payload) -> {
                byte[] name = payload.channel.getBytes(StandardCharsets.UTF_8);
                buf.writeShort(name.length);
                buf.writeBytes(name);
                buf.writeBytes(payload.data);
            },
            buf -> {
                byte[] name = new byte[buf.readUnsignedShort()];
                buf.readBytes(name);
                byte[] data = new byte[buf.readableBytes()];
                buf.readBytes(data);
                return new YogPayload(new String(name, StandardCharsets.UTF_8), data);
            });

    /** A message that came to the server from {@code player}: to the mods. */
    public void toServer(net.minecraft.server.level.ServerPlayer player) {
        new dev.yog.bus.YogEvents.Server.Net.Packet(channel, player.getName().getString(), data).emit();
    }

    /** A message that came to this client: to the mods. */
    public void toClient() {
        new dev.yog.bus.YogEvents.Client.Net.Packet(channel, data).emit();
    }

    @Override
    public Type<? extends CustomPacketPayload> type() {
        return TYPE;
    }
}
