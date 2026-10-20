package dev.yog;

/** What the arena test asks of Java: handlers to provide, calls to make. */
public final class ArenaProbe {
    private ArenaProbe() {}

    /** Answers {@code address} with the payload backwards. */
    public static void provideEcho(String address) {
        YogBus.provide(YogBus.resolve(address), p -> YogBus.Reply.ok(reversed(p)));
    }

    /** Answers {@code address} with {@code size} bytes, whatever it's asked. */
    public static void provideBig(String address, int size) {
        YogBus.provide(YogBus.resolve(address), p -> YogBus.Reply.ok(new byte[size]));
    }

    /** Calls {@code address}: the status, then the reply. */
    public static byte[] call(String address, byte[] payload) {
        YogBus.Reply r = YogBus.call(YogBus.resolve(address), payload);
        byte[] out = new byte[1 + r.bytes().length];
        out[0] = (byte) r.status();
        System.arraycopy(r.bytes(), 0, out, 1, r.bytes().length);
        return out;
    }

    private static byte[] reversed(byte[] p) {
        byte[] out = new byte[p.length];
        for (int i = 0; i < p.length; i++) out[i] = p[p.length - 1 - i];
        return out;
    }
}
