import java.nio.file.Files;
import java.nio.file.Path;

/** `FrameTrip Record in out`: reads frame `in` as `YogTypes.Record` and packs it again into `out`. */
public final class FrameTrip {
    public static void main(String[] args) throws Exception {
        Class<?> record = Class.forName("dev.yog.bus.YogTypes$" + args[0]);
        byte[] in = Files.readAllBytes(Path.of(args[1]));
        Object value = record.getMethod("fromFrame", byte[].class).invoke(null, (Object) in);
        Files.write(Path.of(args[2]), (byte[]) record.getMethod("toFrame").invoke(value));
    }
}
