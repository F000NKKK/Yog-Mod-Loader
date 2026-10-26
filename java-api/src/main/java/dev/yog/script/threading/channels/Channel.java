package dev.yog.script.threading.channels;

/** Yog Script's {@code System.Threading.Channels.Channel}: channels made. */
public final class Channel {
    private Channel() {}

    public static <T> Channel1<T> CreateUnbounded() {
        return new Channel1<>(Integer.MAX_VALUE, BoundedChannelFullMode.Wait);
    }

    public static <T> Channel1<T> CreateBounded(int capacity) {
        return CreateBounded(capacity, BoundedChannelFullMode.Wait);
    }

    public static <T> Channel1<T> CreateBounded(int capacity, BoundedChannelFullMode fullMode) {
        if (capacity < 1) {
            throw new dev.yog.script.YsPanic("a bounded channel holds at least one item", "Channel.CreateBounded");
        }
        return new Channel1<>(capacity, fullMode);
    }
}
