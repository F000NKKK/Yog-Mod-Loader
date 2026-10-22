package net.fakegame;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.Map;
import java.util.function.BiConsumer;

/** The game's pack types, as their shapes: a supplier of a resource's stream, a listing's output. */
public final class FakePack {
    private FakePack() {}

    public interface Supplier {
        InputStream open() throws IOException;
    }

    public interface Output extends BiConsumer<String, Supplier> {}

    public interface Resources {
        Supplier resource(String path);

        void list(String prefix, Output out);
    }

    /** Resources holding {@code files}. */
    public static Resources of(Map<String, byte[]> files) {
        return new Resources() {
            @Override
            public Supplier resource(String path) {
                byte[] b = files.get(path);
                return b == null ? null : () -> new ByteArrayInputStream(b);
            }

            @Override
            public void list(String prefix, Output out) {
                files.forEach((p, b) -> {
                    if (p.startsWith(prefix)) out.accept(p, () -> new ByteArrayInputStream(b));
                });
            }
        };
    }
}
