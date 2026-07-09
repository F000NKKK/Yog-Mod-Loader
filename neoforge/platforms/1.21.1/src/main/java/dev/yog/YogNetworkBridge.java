package dev.yog;

import net.minecraft.server.level.ServerPlayer;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.network.PacketDistributor;
import net.neoforged.neoforge.network.event.RegisterPayloadHandlersEvent;

/**
 * Registers the single bidirectional `YogPayload` type that multiplexes
 * every one of Yog's dynamic, mod-declared packet channels, and provides the
 * send-side helpers `NativeBridge`/`YogClient` call into. See `YogPayload`'s
 * doc comment for why one type instead of one per channel — mirrors the
 * `YogNetworkBridge` on Forge 1.20.1/1.21.1, just built on NeoForge's
 * `RegisterPayloadHandlersEvent`/`PacketDistributor` API instead of
 * `SimpleChannel`/`EventNetworkChannel`.
 */
public final class YogNetworkBridge {
    private YogNetworkBridge() {}

    public static void init(IEventBus modBus) {
        modBus.addListener(YogNetworkBridge::onRegisterPayloads);
    }

    private static void onRegisterPayloads(RegisterPayloadHandlersEvent event) {
        event.registrar("1").playBidirectional(YogPayload.TYPE, YogPayload.CODEC, (payload, context) -> {
            if (context.player() instanceof ServerPlayer sp) {
                context.enqueueWork(() ->
                        NativeBridge.nativeOnPacket(payload.channelName(), sp.getName().getString(), payload.data()));
            } else {
                context.enqueueWork(() -> NativeBridge.nativeOnClientPacket(payload.channelName(), payload.data()));
            }
        });
    }

    /** Server -> one specific client. */
    public static boolean sendToPlayer(ServerPlayer player, String channelName, byte[] data) {
        PacketDistributor.sendToPlayer(player, new YogPayload(channelName, data));
        return true;
    }

    /** Client -> server. */
    public static boolean sendToServer(String channelName, byte[] data) {
        PacketDistributor.sendToServer(new YogPayload(channelName, data));
        return true;
    }
}
