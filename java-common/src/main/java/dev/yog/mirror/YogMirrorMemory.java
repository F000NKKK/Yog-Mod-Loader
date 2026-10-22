package dev.yog.mirror;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/**
 * The runtime's memory mirrors are kept in ({@code yog_abi::mirror}), seen
 * as direct buffers: no copy, the same bytes Rust reads and writes.
 */
public final class YogMirrorMemory {
    private YogMirrorMemory() {}

    /** {@code length} bytes at {@code address}, little-endian; null for none. */
    public static ByteBuffer buffer(long address, long length) {
        ByteBuffer b = nativeBuffer(address, length);
        return b == null ? null : b.order(ByteOrder.LITTLE_ENDIAN);
    }

    private static native ByteBuffer nativeBuffer(long address, long length);
}
