package dev.yog.client;

import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.AudioStream;

/**
 * Sounds this client plays for the mods: kept by handle till they end, to
 * stop; a streamed one started partway when asked (a video's sound from
 * where the video is) — the stream the game opens for it skips ahead.
 */
public final class YogSounds {
    private static final Map<Integer, SoundInstance> PLAYING = new ConcurrentHashMap<>();
    private static final AtomicInteger NEXT = new AtomicInteger(1);
    /** Seconds the stream opened next skips: set on the render thread around one play. */
    private static final ThreadLocal<Float> SKIP = new ThreadLocal<>();

    private YogSounds() {}

    /** Plays {@code s} from {@code from} seconds (a streamed sound's; others play whole): its handle. */
    public static int play(SoundInstance s, float from) {
        int handle = NEXT.getAndIncrement();
        PLAYING.put(handle, s);
        Minecraft mc = Minecraft.getInstance();
        mc.execute(() -> {
            PLAYING.values().removeIf(other -> other != s && !mc.getSoundManager().isActive(other));
            if (from > 0) SKIP.set(from);
            try {
                mc.getSoundManager().play(s);
            } finally {
                SKIP.remove();
            }
        });
        return handle;
    }

    /** Stops the sound {@code handle} names, if it still plays. */
    public static void stop(int handle) {
        SoundInstance s = PLAYING.remove(handle);
        if (s == null) return;
        Minecraft mc = Minecraft.getInstance();
        mc.execute(() -> mc.getSoundManager().stop(s));
    }

    /** {@code stream}, skipping ahead when the play opening it asked to. */
    public static CompletableFuture<AudioStream> skipped(CompletableFuture<AudioStream> stream) {
        Float from = SKIP.get();
        if (from == null) return stream;
        SKIP.remove();
        return stream.thenApply(s -> YogSkippedStream.of(s, from));
    }
}
