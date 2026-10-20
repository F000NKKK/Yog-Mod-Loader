import dev.yog.data.YogFrame;
import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * `FrameTrip Record in out`: reads frame `in` as `YogTypes.Record` and packs
 * it again into `out` — after checking the record's view of the same frame
 * reads every field the same: nested records as views, lists of numbers
 * item by item, lists of records walked.
 */
public final class FrameTrip {
    public static void main(String[] args) throws Exception {
        Class<?> record = Class.forName("dev.yog.bus.YogTypes$" + args[0]);
        byte[] in = Files.readAllBytes(Path.of(args[1]));
        Object value = record.getMethod("fromFrame", byte[].class).invoke(null, (Object) in);
        Object view = viewOf(record, new YogFrame.Unpacker(in));
        same(value, view, args[0]);
        Files.write(Path.of(args[2]), (byte[]) record.getMethod("toFrame").invoke(value));
    }

    private static Object viewOf(Class<?> record, YogFrame.Unpacker u) throws Exception {
        return Class.forName(record.getName() + "$View").getConstructor(YogFrame.Unpacker.class).newInstance(u);
    }

    /** Throws unless {@code view} reads as {@code value} does. */
    private static void same(Object value, Object view, String at) throws Exception {
        for (RecordComponent c : value.getClass().getRecordComponents()) {
            String name = c.getName();
            Object expected = c.getAccessor().invoke(value);
            Object got = view.getClass().getMethod(name).invoke(view);
            String where = at + "." + name;
            if (expected != null && expected.getClass().isRecord()) {
                same(expected, got, where);
                continue;
            }
            if (!Objects.deepEquals(expected, got)) throw new AssertionError(where + ": " + expected + " vs " + got);
            if (expected != null && expected.getClass().isArray()) items(expected, view, name, where);
            if (expected instanceof List<?> list && !list.isEmpty() && list.get(0).getClass().isRecord()) walked(list, view, name, where);
        }
    }

    /** A number list read item by item, as it was whole. */
    private static void items(Object array, Object view, String name, String where) throws Exception {
        int n = Array.getLength(array);
        if ((int) view.getClass().getMethod(name + "Length").invoke(view) != n) throw new AssertionError(where + ": length");
        Method item = view.getClass().getMethod(name, int.class);
        for (int k = 0; k < n; k++) {
            Object got = item.invoke(view, k);
            if (!Objects.equals(Array.get(array, k), got)) throw new AssertionError(where + "[" + k + "]: " + Array.get(array, k) + " vs " + got);
        }
    }

    /** A list of records walked, each as it was read whole. */
    private static void walked(List<?> list, Object view, String name, String where) throws Exception {
        List<Object> views = new ArrayList<>();
        String each = "forEach" + Character.toUpperCase(name.charAt(0)) + name.substring(1);
        view.getClass().getMethod(each, java.util.function.Consumer.class).invoke(view, (java.util.function.Consumer<Object>) views::add);
        if (views.size() != list.size()) throw new AssertionError(where + ": walked " + views.size() + " of " + list.size());
        for (int k = 0; k < list.size(); k++) same(list.get(k), views.get(k), where + "[" + k + "]");
    }
}
