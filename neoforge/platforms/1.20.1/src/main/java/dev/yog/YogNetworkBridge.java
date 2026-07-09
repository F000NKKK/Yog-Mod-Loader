package dev.yog;

import java.nio.charset.StandardCharsets;

import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraftforge.network.NetworkEvent;
import net.minecraftforge.network.NetworkRegistry;
import net.minecraftforge.network.PacketDistributor;
import net.minecraftforge.network.simple.SimpleChannel;

/**
 * Single Forge `SimpleChannel` multiplexing every one of Yog's dynamic,
 * mod-declared packet channels — same rationale and framing as the 1.21.1
 * `YogNetworkBridge` (see its doc comment), just built on 1.20.1's
 * `SimpleChannel`/`NetworkRegistry` API instead of `ChannelBuilder`.
 *
 * Previously `NativeBridge`/`YogClient` sent raw `ClientboundCustomPayloadPacket`
 * / `ServerboundCustomPayloadPacket` on an unregistered channel — nothing
 * ever registered a receiver for them, so every packet-based feature
 * (including UI-opening from a block's `on_use_block` handler) silently
 * dropped its packets on this platform too.
 */
public final class YogNetworkBridge {
    private record Msg(String channelName, byte[] data) {}

    private static SimpleChannel channel;

    private YogNetworkBridge() {}

    public static void init() {
        if (channel != null) return;
        channel = NetworkRegistry.newSimpleChannel(
                ResourceLocation.tryParse("yog:bridge"),
                () -> "1", "1"::equals, "1"::equals);

        channel.registerMessage(0, Msg.class,
                (msg, buf) -> {
                    byte[] nameBytes = msg.channelName().getBytes(StandardCharsets.UTF_8);
                    buf.writeShort(nameBytes.length);
                    buf.writeBytes(nameBytes);
                    buf.writeBytes(msg.data());
                },
                buf -> {
                    int nameLen = buf.readUnsignedShort();
                    byte[] nameBytes = new byte[nameLen];
                    buf.readBytes(nameBytes);
                    String name = new String(nameBytes, StandardCharsets.UTF_8);
                    byte[] data = new byte[buf.readableBytes()];
                    buf.readBytes(data);
                    return new Msg(name, data);
                },
                (msg, ctxSupplier) -> {
                    NetworkEvent.Context ctx = ctxSupplier.get();
                    ctx.setPacketHandled(true);
                    ServerPlayer sender = ctx.getSender();
                    if (sender != null) {
                        ctx.enqueueWork(() ->
                                NativeBridge.nativeOnPacket(msg.channelName(), sender.getName().getString(), msg.data()));
                    } else {
                        ctx.enqueueWork(() -> NativeBridge.nativeOnClientPacket(msg.channelName(), msg.data()));
                    }
                });
    }

    /** Server -> one specific client. */
    public static boolean sendToPlayer(ServerPlayer player, String channelName, byte[] data) {
        if (channel == null) return false;
        channel.send(PacketDistributor.PLAYER.with(() -> player), new Msg(channelName, data));
        return true;
    }

    /** Client -> server. */
    public static boolean sendToServer(String channelName, byte[] data) {
        if (channel == null) return false;
        channel.send(PacketDistributor.SERVER.noArg(), new Msg(channelName, data));
        return true;
    }
}
