package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import dev.yog.util.YogBodyWire;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.gui.Font;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.OrderedSubmitNodeCollector;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.block.MovingBlockRenderState;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.gizmos.DrawableGizmoPrimitives;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.state.level.QuadParticleRenderState;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.VoxelShape;
import org.joml.Quaternionf;
import org.jspecify.annotations.Nullable;

/**
 * An entity drawn for a yog-body capture: its models' and its own geometry
 * kept by texture (8 floats a vertex), everything else (names, shadows,
 * items, particles) left out.
 */
final class YogBodyCapture implements SubmitNodeCollector {
    final Map<String, YogBodyWire.Sink> sinks = new LinkedHashMap<>();
    private static final YogBodyWire.Sink NOWHERE = new YogBodyWire.Sink();

    private VertexConsumer keep(RenderType type) {
        String tex = YogBodyWire.textureOf(type);
        return new Keep(tex == null ? NOWHERE : sinks.computeIfAbsent(tex, t -> new YogBodyWire.Sink()));
    }

    @Override
    public OrderedSubmitNodeCollector order(int order) {
        return this;
    }

    @Override
    public <S> void submitModel(Model<? super S> model, S state, PoseStack pose, RenderType renderType, int light, int overlay,
                                int color, @Nullable TextureAtlasSprite sprite, int outlineColor,
                                ModelFeatureRenderer.@Nullable CrumblingOverlay crumbling) {
        model.setupAnim(state);
        model.renderToBuffer(pose, keep(renderType), light, overlay, color);
    }

    @Override
    public void submitCustomGeometry(PoseStack pose, RenderType renderType, CustomGeometryRenderer geometry) {
        geometry.render(pose.last(), keep(renderType));
    }

    @Override public void submitShadow(PoseStack pose, float radius, List<EntityRenderState.ShadowPiece> pieces) {}
    @Override public void submitNameTag(PoseStack pose, @Nullable Vec3 at, int offset, Component name, boolean seeThrough, int light, CameraRenderState camera) {}
    @Override public void submitText(PoseStack pose, float x, float y, FormattedCharSequence text, boolean shadow, Font.DisplayMode mode,
                                     int light, int color, int background, int outline) {}
    @Override public void submitFlame(PoseStack pose, EntityRenderState state, Quaternionf rotation) {}
    @Override public void submitLeash(PoseStack pose, EntityRenderState.LeashState leash) {}
    @Override public void submitMovingBlock(PoseStack pose, MovingBlockRenderState block, int outline) {}
    @Override public void submitBlockModel(PoseStack pose, RenderType type, List<BlockStateModelPart> parts, int[] tints, int light, int overlay, int outline) {}
    @Override public void submitBreakingBlockModel(PoseStack pose, List<BlockStateModelPart> parts, int progress) {}
    @Override public void submitShapeOutline(PoseStack pose, VoxelShape shape, RenderType type, int color, float width, boolean afterTerrain) {}
    @Override public void submitItem(PoseStack pose, ItemDisplayContext context, int light, int overlay, int outline, int[] tints,
                                     List<BakedQuad> quads, ItemStackRenderState.FoilType foil) {}
    @Override public void submitQuadParticleGroup(QuadParticleRenderState particles) {}
    @Override public void submitGizmoPrimitives(DrawableGizmoPrimitives.Group group, CameraRenderState camera, boolean onTop) {}

    /** Keeps each vertex's position, texture coordinates and normal; drops the rest. */
    private record Keep(YogBodyWire.Sink sink) implements VertexConsumer {
        @Override
        public VertexConsumer addVertex(float x, float y, float z) {
            sink.vertex(x, y, z);
            return this;
        }

        @Override
        public VertexConsumer setColor(int r, int g, int b, int a) {
            return this;
        }

        @Override
        public VertexConsumer setColor(int argb) {
            return this;
        }

        @Override
        public VertexConsumer setUv(float u, float v) {
            sink.uv(u, v);
            return this;
        }

        @Override
        public VertexConsumer setUv1(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer setUv2(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer setNormal(float x, float y, float z) {
            sink.normal(x, y, z);
            return this;
        }

        @Override
        public VertexConsumer setLineWidth(float width) {
            return this;
        }
    }
}
