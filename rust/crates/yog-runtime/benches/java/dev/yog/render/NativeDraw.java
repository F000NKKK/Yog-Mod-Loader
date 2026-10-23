package dev.yog.render;

/** Stand-in for the host's NativeDraw: the same shapes, no game behind them. */
public final class NativeDraw {
    private NativeDraw() {}

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {}

    public static void drawText(String text, float x, float y, int color, boolean shadow) {}
}
