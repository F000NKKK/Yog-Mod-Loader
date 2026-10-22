package dev.yog.api;

import dev.yog.bus.YogEvents;
import dev.yog.data.YogFrame;
import java.util.Arrays;
import java.util.List;
import java.util.Optional;

/** What a Java mod does on the bus, with its own records and the generated ones. */
public final class ApiProbe {
    private ApiProbe() {}

    /** Laid out as {@code YogEvents.Log.Record}: the same frame. */
    record MyLog(long level, String message) {}

    record Charge(String player, long amount, List<String> tags, Optional<Long> bonus, int[] slots) {}

    /** A record packs to the generated record's bytes, and reads back. */
    public static String sameFrame() {
        YogFrame.Packer p = new YogFrame.Packer();
        YogRecordFrames.pack(p, new MyLog(3, "hi"), MyLog.class);
        byte[] mine = p.finish();
        byte[] generated = new YogEvents.Log.Record(3, "hi").toFrame();
        YogFrame.Packer q = new YogFrame.Packer();
        Charge c = new Charge("Dev", 21, List.of("a", "b"), Optional.of(5L), new int[] {1, -2});
        YogRecordFrames.pack(q, c, Charge.class);
        YogFrame.Unpacker u = new YogFrame.Unpacker(q.finish());
        Charge back = (Charge) YogRecordFrames.unpack(u, Charge.class);
        boolean same = back.player().equals("Dev") && back.amount() == 21 && back.tags().equals(c.tags())
                && back.bonus().equals(c.bonus()) && Arrays.equals(back.slots(), c.slots());
        return Arrays.equals(mine, generated) + "," + same;
    }

    /** Through the bus: a provider called, a listener changing an event, a generated `on`. */
    public static String roundTrip() {
        YogBus.provide("test:api/charge", Charge.class, c -> c.amount() * 2);
        Long doubled = YogBus.call("test:api/charge", new Charge("Dev", 21, List.of(), Optional.empty(), new int[0]), Long.class);
        YogBus.on("test:api/event", 0, MyLog.class, e -> new MyLog(e.level() + 1, e.message() + "!"));
        MyLog changed = YogBus.emit("test:api/event", MyLog.class, new MyLog(1, "x"));
        YogBus.onPre("test:api/pre", 0, MyLog.class, e -> e.level() < 10);
        boolean allowed = YogBus.emitPre("test:api/pre", new MyLog(1, "x"));
        boolean refused = YogBus.emitPre("test:api/pre", new MyLog(11, "x"));
        YogEvents.Log.Record.on(0, e -> new YogEvents.Log.Record(e.level() + 100, e.message()));
        long typed = new YogEvents.Log.Record(1, "x").emitChanged().level();
        return doubled + "," + changed.level() + changed.message() + "," + allowed + "," + refused + "," + typed;
    }
}
