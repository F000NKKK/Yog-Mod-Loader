package dev.yog;

import java.nio.charset.StandardCharsets;

import io.netty.buffer.Unpooled;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraftforge.event.network.CustomPayloadEvent;
import net.minecraftforge.network.ChannelBuilder;
import net.minecraftforge.network.EventNetworkChannel;
import net.minecraftforge.network.PacketDistributor;

/**
 * Single Forge network channel multiplexing every one of Yog's dynamic,
 * mod-declared packet channels. Forge's typed networking (`ChannelBuilder`
 * + per-message `Type`/`StreamCodec`) wants every message type declared once,
 * up front — but Yog mods register channel *names* at runtime via
 * `on_packet`/`on_client_packet`/`send_to_player`/`send_to_server`, so this
 * wraps every payload as `[u16 name length][name utf8][raw data]` on one
 * `EventNetworkChannel` (raw `FriendlyByteBuf`, no per-name registration) and
 * demultiplexes by name on receipt.
 *
 * Previously `NativeBridge`/`YogClient` sent raw `ClientboundCustomPayloadPacket`
 * / `ServerboundCustomPayloadPacket` directly with no channel registered at
 * all — Forge's networking silently dropped every one of those (this is why
 * no packet-based feature, including UI-opening from a block's `on_use_block`
 * handler, ever worked on this platform).
 */
public final class YogNetworkBridge {
    private static EventNetworkChannel channel;

    private YogNetworkBridge() {}

    public static void init() {
        if (channel != null) return;
        channel = ChannelBuilder.named(ResourceLocation.fromNamespaceAndPath("yog", "bridge"))
                .networkProtocolVersion(1)
                .optional()
                .eventNetworkChannel();
        channel.addListener(YogNetworkBridge::onPayload);
    }

    private static void onPayload(CustomPayloadEvent event) {
        FriendlyByteBuf buf = event.getPayload();
        if (buf == null) return;
        CustomPayloadEvent.Context ctx = event.getSource();

        int nameLen = buf.readUnsignedShort();
        byte[] nameBytes = new byte[nameLen];
        buf.readBytes(nameBytes);
        String channelName = new String(nameBytes, StandardCharsets.UTF_8);
        byte[] data = new byte[buf.readableBytes()];
        buf.readBytes(data);
        ctx.setPacketHandled(true);

        if (ctx.isServerSide()) {
            ServerPlayer sp = ctx.getSender();
            if (sp == null) return;
            String playerName = sp.getName().getString();
            ctx.enqueueWork(() -> NativeBridge.nativeOnPacket(channelName, playerName, data));
        } else {
            ctx.enqueueWork(() -> NativeBridge.nativeOnClientPacket(channelName, data));
        }
    }

    private static FriendlyByteBuf frame(String channelName, byte[] data) {
        byte[] nameBytes = channelName.getBytes(StandardCharsets.UTF_8);
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeShort(nameBytes.length);
        buf.writeBytes(nameBytes);
        buf.writeBytes(data);
        return buf;
    }

    /** Server -> one specific client. */
    public static boolean sendToPlayer(ServerPlayer player, String channelName, byte[] data) {
        if (channel == null) return false;
        channel.send(frame(channelName, data), PacketDistributor.PLAYER.with(player));
        return true;
    }

    /** Client -> server. */
    public static boolean sendToServer(String channelName, byte[] data) {
        if (channel == null) return false;
        channel.send(frame(channelName, data), PacketDistributor.SERVER.noArg());
        return true;
    }
}
