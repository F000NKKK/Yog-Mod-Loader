import dev.yog.data.YogData;
import java.nio.file.Files;
import java.nio.file.Path;

/** `RoundTrip in out`: decodes `in` and writes it encoded again to `out`; every truncation of it must fail to decode cleanly. */
public final class RoundTrip {
    public static void main(String[] args) throws Exception {
        byte[] in = Files.readAllBytes(Path.of(args[0]));
        Files.write(Path.of(args[1]), YogData.encode(YogData.decode(in)));
        for (int len = 0; len < Math.min(in.length, 4096); len++) {
            byte[] cut = java.util.Arrays.copyOf(in, len);
            try {
                YogData.decode(cut);
                throw new AssertionError("a truncation to " + len + " bytes decoded");
            } catch (YogData.DecodeException expected) {
                // Clean failure.
            }
        }
    }
}
