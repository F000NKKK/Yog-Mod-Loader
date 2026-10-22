package dev.yog.util;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.fakegame.FakePack;

/** A pack's yimages read as PNGs through {@link YogImagePack}'s wrapping. */
public final class ImageProbe {
    private ImageProbe() {}

    private static final byte[] PNG = {(byte) 0x89, 'P', 'N', 'G'};

    private static boolean isPng(byte[] b) {
        return b.length >= PNG.length && java.util.Arrays.equals(java.util.Arrays.copyOf(b, PNG.length), PNG);
    }

    /** The yimage {@code yimage} and a PNG {@code png}: what comes out of the wrapped pack. */
    public static String run(byte[] yimage, byte[] png) throws IOException {
        FakePack.Resources pack = YogImagePack.wrap(FakePack.of(Map.of("a.png", yimage, "b.png", png)), FakePack.Resources.class);
        byte[] a = pack.resource("a.png").open().readAllBytes();
        byte[] b = pack.resource("b.png").open().readAllBytes();
        List<Boolean> listed = new ArrayList<>();
        pack.list("a", (path, supplier) -> {
            try {
                listed.add(isPng(supplier.open().readAllBytes()));
            } catch (IOException e) {
                throw new java.io.UncheckedIOException(e);
            }
        });
        return isPng(a) + "," + java.util.Arrays.equals(b, png) + "," + listed + "," + (pack.resource("none.png") == null);
    }
}
