package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.fabric.api.networking.v1.PayloadTypeRegistry;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;

/**
 * Raw-byte custom payload for dynamic Yog channels. Since 1.20.5 Minecraft
 * networking is typed: every channel must register a payload codec up front,
 * so both hosts register one YogPayload codec per Rust-declared channel.
 */
public final class YogPayload implements CustomPacketPayload {
    private static final Map<Identifier, CustomPacketPayload.Type<YogPayload>> IDS = new ConcurrentHashMap<>();

    private final CustomPacketPayload.Type<YogPayload> id;
    private final byte[] data;

    public YogPayload(CustomPacketPayload.Type<YogPayload> id, byte[] data) {
        this.id = id;
        this.data = data;
    }

    public byte[] data() {
        return data;
    }

    public static CustomPacketPayload.Type<YogPayload> idFor(Identifier channel) {
        return IDS.computeIfAbsent(channel, CustomPacketPayload.Type::new);
    }

    /** Register the codec for a channel in both directions (idempotent). */
    public static void register(Identifier channel) {
        CustomPacketPayload.Type<YogPayload> id = idFor(channel);
        StreamCodec<FriendlyByteBuf, YogPayload> codec = StreamCodec.ofMember(
                (payload, buf) -> buf.writeBytes(payload.data),
                buf -> {
                    byte[] d = new byte[buf.readableBytes()];
                    buf.readBytes(d);
                    return new YogPayload(id, d);
                });
        try {
            PayloadTypeRegistry.clientboundPlay().register(id, codec);
        } catch (RuntimeException ignored) {
        }
        try {
            PayloadTypeRegistry.serverboundPlay().register(id, codec);
        } catch (RuntimeException ignored) {
        }
    }

    @Override
    public CustomPacketPayload.Type<? extends CustomPacketPayload> type() {
        return id;
    }
}
