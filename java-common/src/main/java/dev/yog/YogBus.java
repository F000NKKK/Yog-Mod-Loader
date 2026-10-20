package dev.yog;

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

    /** A handler: the payload, and what it answers. */
    @FunctionalInterface
    public interface Handler {
        Reply handle(byte[] payload);
    }

    /** A status and the reply's bytes. */
    public record Reply(int status, byte[] bytes) {
        private static final byte[] NONE = new byte[0];

        public static Reply ok(byte[] bytes) {
            return new Reply(OK, bytes);
        }

        public static Reply ok() {
            return new Reply(OK, NONE);
        }

        public static Reply of(int status) {
            return new Reply(status, NONE);
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

    /** Calls {@code to}'s provider. */
    public static Reply call(int to, byte[] payload) {
        Reply r = viaArena(to, payload, true);
        return r != null ? r : unpack(nativeCall(to, payload));
    }

    /** Runs {@code event}'s handlers; the reply is the last payload (a cancel stops them). */
    public static Reply emit(int event, byte[] payload) {
        Reply r = viaArena(event, payload, false);
        return r != null ? r : unpack(nativeEmit(event, payload));
    }

    /** {@code payload} to {@code address} through this thread's arena; null if it doesn't fit. */
    private static Reply viaArena(int address, byte[] payload, boolean call) {
        ByteBuffer a = ARENA.get();
        if (a == null) return null;
        int at = a.getInt(ARENA_TOP);
        if (ARENA_HEADER + at + payload.length > a.capacity()) return null;
        a.put(ARENA_HEADER + at, payload);
        a.putInt(ARENA_TOP, at + payload.length);
        long answer;
        try {
            answer = call ? nativeCallAt(address, at, payload.length) : nativeEmitAt(address, at, payload.length);
        } finally {
            a.putInt(ARENA_TOP, at);
        }
        int len = (int) answer;
        byte[] bytes;
        if (len == SPILLED) {
            bytes = nativeTakeReply();
        } else {
            bytes = new byte[len];
            a.get(ARENA_HEADER + at + payload.length, bytes);
        }
        return new Reply((int) (answer >>> STATUS_SHIFT), bytes == null ? new byte[0] : bytes);
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

    private static Reply unpack(byte[] packed) {
        if (packed == null || packed.length < STATUS_LEN) return Reply.of(FAILED);
        byte[] bytes = new byte[packed.length - STATUS_LEN];
        System.arraycopy(packed, STATUS_LEN, bytes, 0, bytes.length);
        return new Reply(packed[0] & 0xFF, bytes);
    }

    private static byte[] pack(Reply reply) {
        byte[] out = new byte[STATUS_LEN + reply.bytes().length];
        out[0] = (byte) reply.status();
        System.arraycopy(reply.bytes(), 0, out, STATUS_LEN, reply.bytes().length);
        return out;
    }

    /** Called by the runtime: runs Java handler {@code handler} (a payload too big for the arena). */
    public static byte[] handle(int handler, byte[] payload) {
        return pack(run(handler, payload));
    }

    /**
     * Called by the runtime: runs Java handler {@code handler} on the
     * payload at {@code at} in this thread's arena, {@code len} long; the
     * reply goes right after it (or apart, {@link #takeReply}).
     */
    public static long handleAt(int handler, int at, int len) {
        ByteBuffer a = ARENA.get();
        byte[] payload = new byte[len];
        a.get(ARENA_HEADER + at, payload);
        Reply r = run(handler, payload);
        long status = (long) r.status() << STATUS_SHIFT;
        int out = at + len;
        if (ARENA_HEADER + out + r.bytes().length <= a.capacity()) {
            a.put(ARENA_HEADER + out, r.bytes());
            return status | r.bytes().length;
        }
        SPILL.set(r.bytes());
        return status | LENGTH_MASK;
    }

    /** Called by the runtime: the reply {@link #handleAt} kept apart. */
    public static byte[] takeReply() {
        byte[] b = SPILL.get();
        SPILL.remove();
        return b;
    }

    private static Reply run(int handler, byte[] payload) {
        Handler h = HANDLERS.get(handler);
        if (h == null) return Reply.of(NO_HANDLER);
        try {
            return h.handle(payload);
        } catch (RuntimeException e) {
            YogLog.error("bus handler failed", e);
            return Reply.of(FAILED);
        }
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
