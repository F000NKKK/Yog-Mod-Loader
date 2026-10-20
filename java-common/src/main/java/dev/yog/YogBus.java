package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The bus ({@code docs/design/bus.md}) from Java: the host and Java mods
 * call addresses, emit events, provide services and listen to events — the
 * same channel Yog mods use, so either side reaches the other. A message is
 * an address and a payload in Yog Data's binary form
 * ({@link dev.yog.data.YogData#encode} / {@code decode}).
 */
public final class YogBus {
    private YogBus() {}

    /** How a call or a handler ended — {@code yog_abi::status}'s values (a test keeps them equal). */
    public static final int OK = 0;
    public static final int NO_HANDLER = 1;
    public static final int BAD_PAYLOAD = 2;
    public static final int FAILED = 3;
    public static final int CANCELLED = 4;

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
        return unpack(nativeCall(to, payload));
    }

    /** Runs {@code event}'s handlers; the reply is the last payload (a cancel stops them). */
    public static Reply emit(int event, byte[] payload) {
        return unpack(nativeEmit(event, payload));
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

    /** Called by the runtime: runs Java handler {@code handler}. */
    public static byte[] handle(int handler, byte[] payload) {
        Handler h = HANDLERS.get(handler);
        if (h == null) return pack(Reply.of(NO_HANDLER));
        try {
            return pack(h.handle(payload));
        } catch (RuntimeException e) {
            YogLog.error("bus handler failed", e);
            return pack(Reply.of(FAILED));
        }
    }

    private static native int nativeResolve(String address);

    private static native byte[] nativeCall(int to, byte[] payload);

    private static native byte[] nativeEmit(int event, byte[] payload);

    private static native boolean nativeProvide(int address, int handler);

    private static native void nativeOn(int event, int priority, int handler);

    private static native boolean nativeHeard(int event);
}
