package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.fabric.api.networking.v1.PayloadTypeRegistry;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.network.codec.PacketCodec;
import net.minecraft.network.packet.CustomPayload;
import net.minecraft.util.Identifier;

/**
 * Raw-byte custom payload for dynamic Yog channels. Since 1.20.5 Minecraft
 * networking is typed: every channel must register a payload codec up front,
 * so both hosts register one YogPayload codec per Rust-declared channel.
 */
public final class YogPayload implements CustomPayload {
    private static final Map<Identifier, CustomPayload.Id<YogPayload>> IDS = new ConcurrentHashMap<>();

    private final CustomPayload.Id<YogPayload> id;
    private final byte[] data;

    public YogPayload(CustomPayload.Id<YogPayload> id, byte[] data) {
        this.id = id;
        this.data = data;
    }

    public byte[] data() {
        return data;
    }

    public static CustomPayload.Id<YogPayload> idFor(Identifier channel) {
        return IDS.computeIfAbsent(channel, CustomPayload.Id::new);
    }

    /** Register the codec for a channel in both directions (idempotent). */
    public static void register(Identifier channel) {
        CustomPayload.Id<YogPayload> id = idFor(channel);
        PacketCodec<PacketByteBuf, YogPayload> codec = PacketCodec.of(
                (payload, buf) -> buf.writeBytes(payload.data),
                buf -> {
                    byte[] d = new byte[buf.readableBytes()];
                    buf.readBytes(d);
                    return new YogPayload(id, d);
                });
        try {
            PayloadTypeRegistry.playS2C().register(id, codec);
        } catch (RuntimeException ignored) {
        }
        try {
            PayloadTypeRegistry.playC2S().register(id, codec);
        } catch (RuntimeException ignored) {
        }
    }

    @Override
    public CustomPayload.Id<? extends CustomPayload> getId() {
        return id;
    }
}
