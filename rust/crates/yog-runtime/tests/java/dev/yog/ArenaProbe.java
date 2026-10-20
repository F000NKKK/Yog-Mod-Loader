package dev.yog;

import dev.yog.data.YogFrame;

/** What the arena test asks of Java: handlers to provide, calls to make. */
public final class ArenaProbe {
    private ArenaProbe() {}

    /** Answers {@code address} with the payload backwards. */
    public static void provideEcho(String address) {
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            out.bytes(reversed(in.bytes()));
            return YogBus.OK;
        });
    }

    /** Answers {@code address} with {@code size} bytes, whatever it's asked. */
    public static void provideBig(String address, int size) {
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            out.bytes(new byte[size]);
            return YogBus.OK;
        });
    }

    /** Calls {@code address}: the status, then the reply. */
    public static byte[] call(String address, byte[] payload) {
        YogMessage r = YogBus.call(YogBus.resolve(address), payload);
        byte[] reply = r.bytes();
        byte[] out = new byte[1 + reply.length];
        out[0] = (byte) r.status();
        System.arraycopy(reply, 0, out, 1, reply.length);
        return out;
    }

    /** Answers {@code address}'s {name, n} frame, read in the arena, with {"hi " + name, n * 2} packed there. */
    public static void provideGreet(String address) {
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            YogFrame.Unpacker u = in.frame();
            int end = u.begin();
            String name = u.getStr();
            long n = u.getI();
            u.leave(end);
            u.finish();
            YogFrame.Packer p = out.frame();
            int at = p.begin();
            p.putStr("hi " + name);
            p.putI(n * 2);
            p.end(at);
            return YogBus.OK;
        });
    }

    /** Calls {@code address} with the frame {name, n} from this thread's packers: "status:name:n" of the reply. */
    public static String callGreet(String address, String name, long n) {
        YogFrame.Packer p = YogFrame.Packer.take();
        int at = p.begin();
        p.putStr(name);
        p.putI(n);
        p.end(at);
        YogMessage r = YogBus.call(YogBus.resolve(address), p);
        if (!r.ok()) return String.valueOf(r.status());
        YogFrame.Unpacker u = r.frame();
        int end = u.begin();
        String back = u.getStr();
        long m = u.getI();
        u.leave(end);
        u.finish();
        return r.status() + ":" + back + ":" + m;
    }

    private static byte[] reversed(byte[] p) {
        byte[] out = new byte[p.length];
        for (int i = 0; i < p.length; i++) out[i] = p[p.length - 1 - i];
        return out;
    }
}
