package dev.yog.api;

import dev.yog.YogMessage;
import dev.yog.data.YogFrame;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * The bus for Java mods ({@code docs/design/bus.md}): the channel Yog mods
 * and the host talk on. The game's own events and services are typed records
 * already ({@code dev.yog.bus.YogEvents}, {@code YogServices}: each has
 * {@code on}, {@code onPre}, {@code provide}, {@code emit}, {@code call}).
 * Here, any address — a Yog mod's, another Java mod's, your own — with your
 * own records: a record packs to the same frame as a Rust struct with the
 * same fields in the same order, so a Java mod and a Yog mod share messages
 * by declaring them alike.
 *
 * <pre>{@code
 * record Charge(String player, long amount) {}
 * YogBus.provide("mymod:charge", Charge.class, c -> c.amount() * 2);
 * Long doubled = YogBus.call("mymod:charge", new Charge("Dev", 21), Long.class);
 * }</pre>
 */
public final class YogBus {
    private YogBus() {}

    /** A typed listener: the event in; what it returns, when not null, is what the event becomes. */
    @FunctionalInterface
    public interface Listener<T> {
        T on(T event);
    }

    /** A typed listener before the game acts: false cancels what's about to happen. */
    @FunctionalInterface
    public interface PreListener<T> {
        boolean allow(T event);
    }

    /** A typed provider: the request in, the reply out. */
    @FunctionalInterface
    public interface Provider<Q, R> {
        R answer(Q request);
    }

    private static final Map<String, Integer> ADDRESSES = new ConcurrentHashMap<>();

    private static int at(String address) {
        return ADDRESSES.computeIfAbsent(address, dev.yog.YogBus::resolve);
    }

    /** Whether anyone listens at {@code address} — skip building an event no one hears. */
    public static boolean heard(String address) {
        return dev.yog.YogBus.heard(at(address));
    }

    /** Asks {@code address}'s provider: its reply as a {@code reply}, or null when no one answered. */
    public static <R> R call(String address, Object request, Class<R> reply) {
        YogMessage r = dev.yog.YogBus.call(at(address), packed(request));
        if (!r.ok() || r.isEmpty()) return null;
        return read(r, reply);
    }

    /** Emits {@code event} (a {@code type}) at {@code address}: what the listeners made of it (itself when none changed it). */
    public static <E> E emit(String address, Class<E> type, E event) {
        YogMessage r = dev.yog.YogBus.emit(at(address), packed(event));
        if (!r.ok() || r.isEmpty()) return event;
        return read(r, type);
    }

    /** Emits {@code event} before the game does it: false when a listener cancelled it. */
    public static boolean emitPre(String address, Object event) {
        return dev.yog.YogBus.emit(at(address), packed(event)).status() != dev.yog.YogBus.CANCELLED;
    }

    /** Listens at {@code address} (higher {@code priority} first). */
    public static <E> void on(String address, int priority, Class<E> type, Listener<E> listener) {
        dev.yog.YogBus.on(at(address), priority, (in, out) -> {
            E changed = listener.on(read(in, type));
            if (changed != null) YogRecordFrames.pack(out.frame(), changed, changed.getClass());
            return dev.yog.YogBus.OK;
        });
    }

    /** Listens at {@code address} before the game acts: the listener answering false cancels it. */
    public static <E> void onPre(String address, int priority, Class<E> type, PreListener<E> listener) {
        dev.yog.YogBus.on(at(address), priority, (in, out) ->
                listener.allow(read(in, type)) ? dev.yog.YogBus.OK : dev.yog.YogBus.CANCELLED);
    }

    /** Answers calls to {@code address}; false if someone answers them already. */
    public static <Q, R> boolean provide(String address, Class<Q> type, Provider<Q, R> provider) {
        return dev.yog.YogBus.provide(at(address), (in, out) -> {
            R reply = provider.answer(read(in, type));
            if (reply != null) YogRecordFrames.pack(out.frame(), reply, reply.getClass());
            return dev.yog.YogBus.OK;
        });
    }

    /** {@code value} (a record) as a frame's bytes: what a packet carries. */
    public static byte[] bytes(Object value) {
        YogFrame.Packer p = new YogFrame.Packer();
        YogRecordFrames.pack(p, value, value.getClass());
        return p.finish();
    }

    /** A {@code type} (a record) read back from a frame's {@code bytes}. */
    public static <T> T fromBytes(byte[] bytes, Class<T> type) {
        YogFrame.Unpacker u = new YogFrame.Unpacker(bytes);
        Object v = YogRecordFrames.unpack(u, type);
        u.finish();
        return type.cast(v);
    }

    /** {@code value} packed by one of this thread's packers (given back by the bus once sent). */
    private static YogFrame.Packer packed(Object value) {
        YogFrame.Packer p = YogFrame.Packer.take();
        YogRecordFrames.pack(p, value, value.getClass());
        return p;
    }

    private static <T> T read(YogMessage m, Class<T> type) {
        if (type.isPrimitive()) throw new IllegalArgumentException("a message's type is a class: " + type + "'s wrapper, not " + type);
        YogFrame.Unpacker u = m.frame();
        Object v = YogRecordFrames.unpack(u, type);
        u.finish();
        return type.cast(v);
    }
}
