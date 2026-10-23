package dev.yog;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.ByteBuffer;
import javax.sound.sampled.AudioFormat;
import net.minecraft.client.sounds.AudioStream;

/** An audio stream started partway: what comes before is read and dropped. */
final class YogSkippedStream implements AudioStream {
    /** Bytes read at a time while skipping. */
    private static final int CHUNK = 1 << 16;

    private final AudioStream in;
    /** The rest of the buffer the skipping ended in, given first. */
    private ByteBuffer left;

    private YogSkippedStream(AudioStream in) {
        this.in = in;
    }

    /** {@code in} from {@code seconds} on. */
    static AudioStream of(AudioStream in, float seconds) {
        YogSkippedStream s = new YogSkippedStream(in);
        AudioFormat f = in.getFormat();
        long skip = (long) (seconds * f.getFrameRate()) * f.getFrameSize();
        try {
            while (skip > 0) {
                ByteBuffer b = in.read(CHUNK);
                if (b == null || !b.hasRemaining()) break;
                if (b.remaining() > skip) {
                    b.position(b.position() + (int) skip);
                    s.left = b;
                    break;
                }
                skip -= b.remaining();
            }
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        return s;
    }

    @Override
    public AudioFormat getFormat() {
        return in.getFormat();
    }

    @Override
    public ByteBuffer read(int size) throws IOException {
        if (left != null) {
            ByteBuffer b = left;
            left = null;
            return b;
        }
        return in.read(size);
    }

    @Override
    public void close() throws IOException {
        in.close();
    }
}
