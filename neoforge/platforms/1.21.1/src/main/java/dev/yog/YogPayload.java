package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceLocation;

/**
 * Raw-byte custom payload for dynamic Yog channels.
 * NeoForge 1.21.x uses typed custom payloads — one codec per channel.
 */
public final class YogPayload implements CustomPacketPayload {
    private static final Map<ResourceLocation, Type<YogPayload>> TYPES = new ConcurrentHashMap<>();

    private final Type<YogPayload> type;
    private final byte[] data;

    public YogPayload(Type<YogPayload> type, byte[] data) {
        this.type = type;
        this.data = data;
    }

    public byte[] data() { return data; }

    public static Type<YogPayload> typeFor(ResourceLocation channel) {
        return TYPES.computeIfAbsent(channel, Type::new);
    }

    @Override
    public Type<? extends CustomPacketPayload> type() { return type; }

    /** StreamCodec for this payload type on a specific channel. */
    public static StreamCodec<FriendlyByteBuf, YogPayload> codecFor(ResourceLocation channel) {
        Type<YogPayload> t = typeFor(channel);
        return StreamCodec.of(
                (buf, payload) -> buf.writeBytes(payload.data),
                buf -> {
                    byte[] d = new byte[buf.readableBytes()];
                    buf.readBytes(d);
                    return new YogPayload(t, d);
                });
    }
}
