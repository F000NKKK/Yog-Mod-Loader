package dev.yog;

import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.reflect.Field;
import java.util.List;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.render.pip.PictureInPictureRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.state.gui.pip.PictureInPictureRenderState;
import org.jspecify.annotations.Nullable;

/**
 * Raw GL a Yog screen or the HUD asked for while the GUI was extracted,
 * drawn when the GUI is drawn: since 1.21.6 the GUI is extracted first and
 * drawn after the frame's other passes, so GL drawn at once would be drawn
 * over. The runtime keeps the calls ({@code gpu_record}); this picture in
 * the picture replays them into its texture, the whole screen's size,
 * which the GUI puts in their place among its layers.
 *
 * <p>A renderer draws one picture a frame (its texture is reused): each
 * layer — the HUD, a screen, its overlay — is a renderer of its own.
 */
final class YogGlPip<T extends YogGlPip.Drawn> extends PictureInPictureRenderer<T> {
    /** What a layer draws: a recording, over the whole screen. */
    interface Drawn extends PictureInPictureRenderState {
        long recording();
    }

    record Layer0(long recording, int x0, int y0, int x1, int y1, float scale,
                  @Nullable ScreenRectangle scissorArea, @Nullable ScreenRectangle bounds) implements Drawn {}
    record Layer1(long recording, int x0, int y0, int x1, int y1, float scale,
                  @Nullable ScreenRectangle scissorArea, @Nullable ScreenRectangle bounds) implements Drawn {}
    record Layer2(long recording, int x0, int y0, int x1, int y1, float scale,
                  @Nullable ScreenRectangle scissorArea, @Nullable ScreenRectangle bounds) implements Drawn {}
    record Layer3(long recording, int x0, int y0, int x1, int y1, float scale,
                  @Nullable ScreenRectangle scissorArea, @Nullable ScreenRectangle bounds) implements Drawn {}

    /**
     * A layer's renderer. Made only once the game has its device (a
     * renderer allocates on it): registered as a factory, not made up front.
     */
    static YogGlPip<?> of(Class<? extends Drawn> layer) {
        return new YogGlPip<>(layer);
    }

    /** The layers' state classes, in order. */
    static final List<Class<? extends Drawn>> LAYERS = List.of(Layer0.class, Layer1.class, Layer2.class, Layer3.class);
    /** The picture's scale: its texture is the screen's own pixels. */
    private static final float SCALE = 1f;
    /** The next layer to use: round and round (a frame has fewer than there are). */
    private static int next;

    private final Class<T> type;

    YogGlPip(Class<T> type) {
        this.type = type;
    }

    @Override
    public Class<T> getRenderStateClass() {
        return type;
    }

    @Override
    protected void renderToTexture(T s, PoseStack pose, SubmitNodeCollector out) {
        GpuTexture color = texture(COLOR), depth = texture(DEPTH);
        if (color == null || depth == null) return;
        if (YogGl.beginOn(color, depth, color.getWidth(0), color.getHeight(0))) {
            NativeBridge.nativeGfxReplay(s.recording());
            YogGl.end();
        }
    }

    @Override
    protected String getTextureLabel() {
        return "yog gl";
    }

    /** Draws recording {@code id} ({@code w}×{@code h} GUI pixels) where {@code ctx} is at. */
    static void add(GuiGraphicsExtractor ctx, long id, int w, int h) {
        if (id == 0) return;
        ScreenRectangle all = new ScreenRectangle(0, 0, w, h);
        Drawn d = switch (next++ % LAYERS.size()) {
            case 0 -> new Layer0(id, 0, 0, w, h, SCALE, null, all);
            case 1 -> new Layer1(id, 0, 0, w, h, SCALE, null, all);
            case 2 -> new Layer2(id, 0, 0, w, h, SCALE, null, all);
            default -> new Layer3(id, 0, 0, w, h, SCALE, null, all);
        };
        ctx.guiRenderState.addPicturesInPictureState(d);
    }

    // The picture's own textures: the game keeps them to itself.
    private static final Field COLOR = field("texture");
    private static final Field DEPTH = field("depthTexture");

    private static Field field(String name) {
        try {
            Field f = PictureInPictureRenderer.class.getDeclaredField(name);
            f.setAccessible(true);
            return f;
        } catch (ReflectiveOperationException e) {
            YogLog.warn("a picture in the picture's " + name + " isn't to be found: " + e);
            return null;
        }
    }

    private @Nullable GpuTexture texture(@Nullable Field f) {
        try {
            return f == null ? null : (GpuTexture) f.get(this);
        } catch (ReflectiveOperationException e) {
            return null;
        }
    }
}
