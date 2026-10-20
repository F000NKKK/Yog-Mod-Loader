package dev.yog;

import dev.yog.data.YogFrame;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The bus ({@code docs/design/bus.md}) from Java: the host and Java mods
 * call addresses, emit events, provide services and listen to events — the
 * same channel Yog mods use, so either side reaches the other. A message is
 * an address and a payload: a frame ({@link dev.yog.data.YogFrame}).
 *
 * <p>A call crosses through this thread's arena — memory the runtime owns,
 * seen here as a direct buffer: the payload written at its top, only
 * numbers passed across, the reply written right after the payload and read
 * in place ({@code yog-runtime}'s {@code bus/arena.rs}). What doesn't fit
 * goes as a {@code byte[]}.
 */
public final class YogBus {
    private YogBus() {}

    /** How a call or a handler ended — {@code yog_abi::status}'s values (a test keeps them equal). */
    public static final int OK = 0;
    public static final int NO_HANDLER = 1;
    public static final int BAD_PAYLOAD = 2;
    public static final int FAILED = 3;
    public static final int CANCELLED = 4;

    /** Where the arena's frames start, and where its top (a little-endian int) is — {@code bus/arena.rs}'s {@code HEADER}, {@code TOP}. */
    public static final int ARENA_HEADER = 16;
    public static final int ARENA_TOP = 0;
    /** A reply length saying the reply didn't fit the arena: it's taken apart. */
    private static final int SPILLED = -1;
    /** An answer: the status in the high half, the reply's length in the low. */
    private static final int STATUS_SHIFT = 32;
    private static final long LENGTH_MASK = 0xFFFFFFFFL;

    /** This thread's arena (null where the runtime gives none). */
    private static final ThreadLocal<ByteBuffer> ARENA = ThreadLocal.withInitial(() -> {
        ByteBuffer b = nativeArena();
        return b == null ? null : b.order(ByteOrder.LITTLE_ENDIAN);
    });
    /** A reply too big for the arena, until the runtime takes it. */
    private static final ThreadLocal<byte[]> SPILL = new ThreadLocal<>();

    /** A handler: the message, where its reply goes; it answers a status. */
    @FunctionalInterface
    public interface Handler {
        int handle(YogMessage in, Out out);
    }

    /**
     * Where a handler's reply goes: a frame packed straight into the arena
     * after the payload, or bytes as they are. Nothing written: an empty reply.
     */
    public static final class Out {
        private final ByteBuffer arena;
        private final int at;
        private YogFrame.Packer packer;
        private byte[] raw;

        Out(ByteBuffer arena, int at) {
            this.arena = arena;
            this.at = at;
        }

        /** The reply's packer (this thread's, given back when the handler is done). */
        public YogFrame.Packer frame() {
            if (packer == null) packer = YogFrame.Packer.take();
            return packer;
        }

        /** The reply: {@code bytes} as they are. */
        public void bytes(byte[] bytes) {
            raw = bytes;
        }

        /** Written into the arena: its length, or {@link #SPILLED} with the bytes kept apart. */
        int place() {
            try {
                if (packer != null && !packer.isEmpty()) {
                    int len = packer.size();
                    if (fits(len)) {
                        packer.writeTo(arena, at);
                        return len;
                    }
                    raw = packer.finish();
                }
                if (raw == null) return 0;
                if (fits(raw.length)) {
                    arena.put(at, raw);
                    return raw.length;
                }
                SPILL.set(raw);
                return SPILLED;
            } finally {
                release();
            }
        }

        /** The reply's bytes, apart. */
        byte[] taken() {
            try {
                if (packer != null && !packer.isEmpty()) return packer.finish();
                return raw == null ? NO_BYTES : raw;
            } finally {
                release();
            }
        }

        /** Whatever was written goes: the handler didn't answer. */
        void discard() {
            release();
            raw = null;
        }

        private boolean fits(int len) {
            return arena != null && at + len <= arena.capacity();
        }

        private void release() {
            if (packer != null) packer.release();
            packer = null;
        }
    }

    private static final Map<Integer, Handler> HANDLERS = new ConcurrentHashMap<>();
    private static final AtomicInteger NEXT_HANDLER = new AtomicInteger();

    /** {@code address}'s handle — the same for the runtime's lifetime; resolve once, keep it. */
    public static int resolve(String address) {
        return nativeResolve(address);
    }

    private static final Map<String, Integer> KEYED = new ConcurrentHashMap<>();

    /**
     * {@code address}'s branch for {@code key} — where a keyed event with that
     * key goes ({@code yog:event/entity/game/tick/mymod:golem}); resolved once.
     */
    public static int keyed(String address, String key) {
        return KEYED.computeIfAbsent(address + YogNames.Sep.PATH + key, YogBus::resolve);
    }

    /** Whether anyone listens to {@code event} — skip building a payload no one hears. */
    public static boolean heard(int event) {
        return nativeHeard(event);
    }

    /** Calls {@code to}'s provider with the frame {@code payload} packed (given back to its pool). */
    public static YogMessage call(int to, YogFrame.Packer payload) {
        return send(to, payload, true);
    }

    /** Calls {@code to}'s provider with {@code payload} as it is. */
    public static YogMessage call(int to, byte[] payload) {
        YogMessage r = viaArena(to, payload, null, true);
        return r != null ? r : unpack(nativeCall(to, payload));
    }

    /** Runs {@code event}'s handlers on the frame {@code payload}; the reply is the payload they changed (empty: none did). */
    public static YogMessage emit(int event, YogFrame.Packer payload) {
        return send(event, payload, false);
    }

    /** {@link #emit(int, YogFrame.Packer)} with {@code payload} as it is. */
    public static YogMessage emit(int event, byte[] payload) {
        YogMessage r = viaArena(event, payload, null, false);
        return r != null ? r : unpack(nativeEmit(event, payload));
    }

    private static YogMessage send(int address, YogFrame.Packer payload, boolean call) {
        try {
            YogMessage r = viaArena(address, null, payload, call);
            if (r != null) return r;
            byte[] bytes = payload.finish();
            return unpack(call ? nativeCall(address, bytes) : nativeEmit(address, bytes));
        } finally {
            payload.release();
        }
    }

    /**
     * The payload — {@code bytes}, or else {@code frame} packed in place — to
     * {@code address} through this thread's arena, the reply read where it
     * was written; null if there's no room.
     */
    private static YogMessage viaArena(int address, byte[] bytes, YogFrame.Packer frame, boolean call) {
        ByteBuffer a = ARENA.get();
        if (a == null) return null;
        int at = a.getInt(ARENA_TOP);
        int len = bytes != null ? bytes.length : frame.size();
        if (ARENA_HEADER + at + len > a.capacity()) return null;
        if (bytes != null) a.put(ARENA_HEADER + at, bytes);
        else frame.writeTo(a, ARENA_HEADER + at);
        a.putInt(ARENA_TOP, at + len);
        long answer;
        try {
            answer = call ? nativeCallAt(address, at, len) : nativeEmitAt(address, at, len);
        } finally {
            a.putInt(ARENA_TOP, at);
        }
        int status = (int) (answer >>> STATUS_SHIFT);
        int replyLen = (int) answer;
        if (replyLen == SPILLED) {
            byte[] spilled = nativeTakeReply();
            return spilled == null ? YogMessage.of(status) : YogMessage.of(status, spilled);
        }
        return replyLen == 0 ? YogMessage.of(status) : new YogMessage(status, a, ARENA_HEADER + at + len, replyLen);
    }

    /** Makes {@code handler} answer calls to {@code address}; false if someone does already. */
    public static boolean provide(int address, Handler handler) {
        return nativeProvide(address, register(handler));
    }

    /** Listens to {@code event} (higher {@code priority} first). */
    public static void on(int event, int priority, Handler handler) {
        nativeOn(event, priority, register(handler));
    }

    private static int register(Handler handler) {
        int id = NEXT_HANDLER.getAndIncrement();
        HANDLERS.put(id, handler);
        return id;
    }

    /** A Java reply's status byte, then its bytes — what crosses to the native side. */
    private static final int STATUS_LEN = 1;
    private static final byte[] NO_BYTES = new byte[0];

    private static YogMessage unpack(byte[] packed) {
        if (packed == null || packed.length < STATUS_LEN) return YogMessage.of(FAILED);
        return new YogMessage(packed[0] & 0xFF, ByteBuffer.wrap(packed), STATUS_LEN, packed.length - STATUS_LEN);
    }

    /** Called by the runtime: runs Java handler {@code handler} (a payload too big for the arena). */
    public static byte[] handle(int handler, byte[] payload) {
        Out out = new Out(null, 0);
        int status = run(handler, YogMessage.of(OK, payload), out);
        byte[] reply = out.taken();
        byte[] packed = new byte[STATUS_LEN + reply.length];
        packed[0] = (byte) status;
        System.arraycopy(reply, 0, packed, STATUS_LEN, reply.length);
        return packed;
    }

    /**
     * Called by the runtime: runs Java handler {@code handler} on the
     * payload at {@code at} in this thread's arena, {@code len} long; the
     * reply goes right after it (or apart, {@link #takeReply}).
     */
    public static long handleAt(int handler, int at, int len) {
        ByteBuffer a = ARENA.get();
        int from = ARENA_HEADER + at;
        Out out = new Out(a, from + len);
        int status = run(handler, new YogMessage(OK, a, from, len), out);
        return (long) status << STATUS_SHIFT | (out.place() & LENGTH_MASK);
    }

    /** Called by the runtime: the reply {@link #handleAt} kept apart. */
    public static byte[] takeReply() {
        byte[] b = SPILL.get();
        SPILL.remove();
        return b;
    }

    private static int run(int handler, YogMessage in, Out out) {
        Handler h = HANDLERS.get(handler);
        if (h == null) return NO_HANDLER;
        int status;
        try {
            status = h.handle(in, out);
        } catch (RuntimeException e) {
            YogLog.error("bus handler failed", e);
            status = FAILED;
        }
        // Only a handler that answered has a reply.
        if (status != OK) out.discard();
        return status;
    }

    private static native int nativeResolve(String address);

    private static native byte[] nativeCall(int to, byte[] payload);

    private static native byte[] nativeEmit(int event, byte[] payload);

    private static native boolean nativeProvide(int address, int handler);

    private static native void nativeOn(int event, int priority, int handler);

    private static native boolean nativeHeard(int event);

    private static native ByteBuffer nativeArena();

    private static native long nativeCallAt(int to, int at, int len);

    private static native long nativeEmitAt(int event, int at, int len);

    private static native byte[] nativeTakeReply();
}
