package dev.yog;

import java.nio.charset.StandardCharsets;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.resources.ResourceLocation;

/**
 * Every one of the mods' packet channels, as one message on Yog's one
 * network channel ({@link #CHANNEL}): mods name their channels at run time
 * (`on_packet`, `send_to_player`, …), so a message carries
 * {@code [u16 name length][name utf8][raw data]} and is handed on by its
 * name. The same on every loader; each opens the channel its own way.
 */
public record YogPayload(String channel, byte[] data) {
    /** Yog's network channel: {@code yog:bridge}. */
    public static final ResourceLocation CHANNEL = new ResourceLocation(YogNames.Ns.YOG, "bridge");

    /** This message into {@code buf}. */
    public void write(FriendlyByteBuf buf) {
        byte[] name = channel.getBytes(StandardCharsets.UTF_8);
        buf.writeShort(name.length);
        buf.writeBytes(name);
        buf.writeBytes(data);
    }

    /** The message in {@code buf}. */
    public static YogPayload read(FriendlyByteBuf buf) {
        byte[] name = new byte[buf.readUnsignedShort()];
        buf.readBytes(name);
        byte[] data = new byte[buf.readableBytes()];
        buf.readBytes(data);
        return new YogPayload(new String(name, StandardCharsets.UTF_8), data);
    }

    /** A message that came to the server from {@code player}: to the mods. */
    public void toServer(net.minecraft.server.level.ServerPlayer player) {
        new dev.yog.bus.YogEvents.Server.Net.Packet(channel, player.getName().getString(), data).emit();
    }

    /** A message that came to this client: to the mods. */
    public void toClient() {
        new dev.yog.bus.YogEvents.Client.Net.Packet(channel, data).emit();
    }
}
