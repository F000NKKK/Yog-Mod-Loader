package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.render.pip.PictureInPictureRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.block.BlockModelRenderState;
import net.minecraft.client.renderer.block.BlockModelResolver;
import net.minecraft.client.renderer.block.model.BlockDisplayContext;
import net.minecraft.client.renderer.state.gui.pip.PictureInPictureRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.world.level.block.state.BlockState;
import org.jspecify.annotations.Nullable;

/**
 * Blocks in 3D in a GUI box ({@link NativeDraw#drawBlocks}): since 1.21.6 the
 * GUI draws 3D into its own texture first, a picture in the picture.
 */
final class YogBlocksPip extends PictureInPictureRenderer<YogBlocksPip.State> {
    /** Light as bright as it gets (sky and block 15), packed as the game does. */
    private static final int FULL_BRIGHT = 0xF000F0;
    /** No outline around the blocks. */
    private static final int NO_OUTLINE = 0;
    private static final BlockDisplayContext DISPLAY = BlockDisplayContext.create();

    /** A block at a whole-block offset. */
    record Placed(int x, int y, int z, BlockState state) {}

    /**
     * What to draw: the blocks centred on {@code center}, {@code scale} GUI
     * pixels per block, turned {@code yaw}, tilted {@code pitch}, in the box (x0, y0)–(x1, y1).
     */
    record State(List<Placed> blocks, float cx, float cy, float cz, float yaw, float pitch, float scale,
                 int x0, int y0, int x1, int y1, @Nullable ScreenRectangle scissorArea, @Nullable ScreenRectangle bounds)
            implements PictureInPictureRenderState {
        State(List<Placed> blocks, float cx, float cy, float cz, float yaw, float pitch, float scale,
              int x0, int y0, int x1, int y1, @Nullable ScreenRectangle scissorArea) {
            this(blocks, cx, cy, cz, yaw, pitch, scale, x0, y0, x1, y1, scissorArea,
                    PictureInPictureRenderState.getBounds(x0, y0, x1, y1, scissorArea));
        }
    }

    private final BlockModelResolver models = new BlockModelResolver(Minecraft.getInstance().getModelManager());
    private final BlockModelRenderState model = new BlockModelRenderState();

    @Override
    public Class<State> getRenderStateClass() {
        return State.class;
    }

    @Override
    protected void renderToTexture(State s, PoseStack pose, SubmitNodeCollector out) {
        pose.mulPose(com.mojang.math.Axis.XP.rotationDegrees(s.pitch()));
        pose.mulPose(com.mojang.math.Axis.YP.rotationDegrees(s.yaw()));
        pose.translate(-s.cx(), -s.cy(), -s.cz());
        for (Placed b : s.blocks()) {
            pose.pushPose();
            pose.translate(b.x(), b.y(), b.z());
            models.update(model, b.state(), DISPLAY);
            model.submit(pose, out, FULL_BRIGHT, OverlayTexture.NO_OVERLAY, NO_OUTLINE);
            pose.popPose();
        }
    }

    @Override
    protected float getTranslateY(int height, int guiScale) {
        return height / 2.0F;
    }

    @Override
    protected String getTextureLabel() {
        return "yog blocks";
    }
}
