package dev.yog.mirror;

/** Writes into a mirror's column the way the host does, for the runtime's JVM test. */
public final class MirrorProbe {
    private MirrorProbe() {}

    public static void writePos(long header, int row, double x, double y, double z) {
        YogColumn c = YogColumn.at(header);
        c.begin();
        c.putDoubles(row, x, y, z);
        c.end();
    }

    public static void setBit(long header, int bit) {
        YogColumn c = YogColumn.at(header);
        c.begin();
        c.putBit(bit, true);
        c.end();
    }
}
