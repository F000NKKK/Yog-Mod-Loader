package dev.yog.content;

import dev.yog.YogNames;

import com.mojang.blaze3d.platform.GlStateManager;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.Tesselator;
import com.mojang.blaze3d.vertex.VertexFormat;
import dev.yog.bus.YogTypes;
import java.util.List;
import net.minecraft.client.particle.ParticleRenderType;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureManager;

/**
 * How the mods' particles are drawn: the game's opaque and translucent
 * particle sheets, and for a particle that blends additively or doesn't
 * write depth, a sheet of Yog's that does — drawn after the game's
 * ({@code ParticleOrderMixin} puts them in its order).
 */
public final class YogParticleLayers {
    private YogParticleLayers() {}

    /** Where Yog's sheets are named: {@code yog:particle_additive_nodepth}. */
    private static final String NAME = YogNames.Ns.YOG + YogNames.Sep.NAMESPACE + "particle_";
    private static final String ADDITIVE = "additive", TRANSLUCENT = "translucent", NO_DEPTH = "_nodepth";

    private static final ParticleRenderType ADDITIVE_DEPTH = new Sheet(ADDITIVE, true, true);
    private static final ParticleRenderType ADDITIVE_NO_DEPTH = new Sheet(ADDITIVE + NO_DEPTH, true, false);
    private static final ParticleRenderType TRANSLUCENT_NO_DEPTH = new Sheet(TRANSLUCENT + NO_DEPTH, false, false);

    /** Yog's sheets, in the order they're drawn. */
    public static final List<ParticleRenderType> ALL = List.of(TRANSLUCENT_NO_DEPTH, ADDITIVE_DEPTH, ADDITIVE_NO_DEPTH);

    /** The sheet drawing as {@code render} says. */
    static ParticleRenderType of(YogTypes.ParticleRender render) {
        if (YogTypes.Blend.OPAQUE.equals(render.blend())) return ParticleRenderType.PARTICLE_SHEET_OPAQUE;
        if (YogTypes.Blend.ADDITIVE.equals(render.blend())) return render.depthWrite() ? ADDITIVE_DEPTH : ADDITIVE_NO_DEPTH;
        return render.depthWrite() ? ParticleRenderType.PARTICLE_SHEET_TRANSLUCENT : TRANSLUCENT_NO_DEPTH;
    }

    /**
     * Draws with the particles' atlas, blending {@code additive}ly (its
     * light added by its alpha, as Hex's conjured particles) or as see-through,
     * writing depth or not.
     */
    private record Sheet(String name, boolean additive, boolean depthWrite) implements ParticleRenderType {
        private void state() {
            RenderSystem.depthMask(depthWrite);
            RenderSystem.setShaderTexture(0, TextureAtlas.LOCATION_PARTICLES);
            RenderSystem.enableBlend();
            RenderSystem.blendFunc(GlStateManager.SourceFactor.SRC_ALPHA,
                    additive ? GlStateManager.DestFactor.ONE : GlStateManager.DestFactor.ONE_MINUS_SRC_ALPHA);
        }

        @Override
        public void begin(BufferBuilder builder, TextureManager textures) {
            state();
            builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.PARTICLE);
        }

        @Override
        public void end(Tesselator tesselator) {
            tesselator.end();
        }

        @Override
        public String toString() {
            return NAME + name;
        }
    }
}
