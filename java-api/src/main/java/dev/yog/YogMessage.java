package dev.yog;

import dev.yog.data.YogFrame;
import java.nio.ByteBuffer;

/**
 * A message on the bus where its bytes are — this thread's arena, or an
 * array — and the status it came with. Read an arena's message before this
 * thread's next bus call: that call reuses the space.
 */
public final class YogMessage {
    private static final ByteBuffer NONE = ByteBuffer.allocate(0);
    private static final YogMessage OK_EMPTY = new YogMessage(YogBus.OK, NONE, 0, 0);

    private final int status;
    private final ByteBuffer buf;
    private final int at;
    private final int length;

    YogMessage(int status, ByteBuffer buf, int at, int length) {
        this.status = status;
        this.buf = buf;
        this.at = at;
        this.length = length;
    }

    /** No bytes, with {@code status}. */
    public static YogMessage of(int status) {
        return status == YogBus.OK ? OK_EMPTY : new YogMessage(status, NONE, 0, 0);
    }

    /** {@code bytes}, answered {@code status}. */
    public static YogMessage of(int status, byte[] bytes) {
        return new YogMessage(status, ByteBuffer.wrap(bytes), 0, bytes.length);
    }

    public int status() {
        return status;
    }

    public boolean ok() {
        return status == YogBus.OK;
    }

    public int length() {
        return length;
    }

    public boolean isEmpty() {
        return length == 0;
    }

    /** The bytes, copied out. */
    public byte[] bytes() {
        byte[] out = new byte[length];
        buf.get(at, out);
        return out;
    }

    /** A reader of the frame it holds, in place. */
    public YogFrame.Unpacker frame() {
        return new YogFrame.Unpacker(buf, at, at + length);
    }
}
