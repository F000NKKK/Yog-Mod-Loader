package dev.yog.util;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

/**
 * How a hanging entity hangs, as the mods read it ({@code yog_api::entities::Hanging}):
 * its entity data {@link #KEY}, which the host keeps up to date. Little-endian;
 * {@code facing} and {@code item} a u16 length and UTF-8, {@code width} and
 * {@code height} u16, {@code nbt} a u32 length and UTF-8.
 */
public final class YogHangingWire {
    private YogHangingWire() {}

    /** The entity data key ({@code yog_api::entities::HANGING}). */
    public static final String KEY = "yog:hanging";

    /** {@code facing}: {@code north}, {@code south}, {@code west} or {@code east}; {@code nbt}: the item's custom data. */
    public static byte[] encode(String facing, int width, int height, String item, String nbt) {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        str16(o, facing);
        u16(o, width);
        u16(o, height);
        str16(o, item);
        byte[] b = nbt.getBytes(StandardCharsets.UTF_8);
        u16(o, b.length);
        u16(o, b.length >>> 16);
        o.writeBytes(b);
        return o.toByteArray();
    }

    private static void u16(ByteArrayOutputStream o, int v) {
        o.write(v & 0xFF);
        o.write(v >>> 8 & 0xFF);
    }

    private static void str16(ByteArrayOutputStream o, String s) {
        byte[] b = s.getBytes(StandardCharsets.UTF_8);
        int n = Math.min(b.length, 0xFFFF);
        u16(o, n);
        o.write(b, 0, n);
    }
}
