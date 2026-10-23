package dev.yog;

import com.mojang.blaze3d.platform.NativeImage;
import dev.yog.mixin.NativeImageAccess;
import org.lwjgl.system.MemoryUtil;

/** RGBA bytes put straight into an image's memory: one copy, no call a pixel. */
public final class YogPixels {
    /** Bytes a pixel takes (an RGBA image keeps them in this order). */
    public static final int RGBA = 4;

    private YogPixels() {}

    /** Whether {@code rgba} holds a {@code w}×{@code h} picture. */
    public static boolean fits(int w, int h, byte[] rgba) {
        return w > 0 && h > 0 && rgba.length >= w * h * RGBA;
    }

    /** {@code img} (RGBA) filled with {@code rgba}; whether it fit. */
    public static boolean fill(NativeImage img, byte[] rgba) {
        if (!fits(img.getWidth(), img.getHeight(), rgba)) return false;
        int n = img.getWidth() * img.getHeight() * RGBA;
        MemoryUtil.memByteBuffer(((NativeImageAccess) (Object) img).yog$pixels(), n).put(rgba, 0, n);
        return true;
    }
}
