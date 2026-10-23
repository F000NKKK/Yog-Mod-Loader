package dev.yog;

import com.mojang.blaze3d.pipeline.BlendFunction;
import com.mojang.blaze3d.pipeline.ColorTargetState;
import com.mojang.blaze3d.pipeline.DepthStencilState;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.vertex.VertexFormat;
import dev.yog.bus.YogTypes;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.particle.SingleQuadParticle;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.ShaderDefines;
import net.minecraft.resources.Identifier;

/**
 * How the mods' particles are drawn: the game's opaque and translucent
 * particle layers, and for a particle that blends otherwise or doesn't
 * write depth, a pipeline made from the game's translucent particles' with
 * that changed — one each way, made once.
 */
final class YogParticleLayers {
    private YogParticleLayers() {}

    /** Where the made pipelines are named: {@code yog:pipeline/particle_additive_nodepth}. */
    private static final String PIPELINE_PATH = "pipeline/particle_";
    private static final String NO_DEPTH = "_nodepth";

    private static final Map<String, SingleQuadParticle.Layer> MADE = new HashMap<>();

    /** The layer drawing as {@code render} says. */
    static SingleQuadParticle.Layer of(YogTypes.ParticleRender render) {
        boolean additive = YogTypes.Blend.ADDITIVE.equals(render.blend());
        if (YogTypes.Blend.OPAQUE.equals(render.blend())) return SingleQuadParticle.Layer.OPAQUE;
        if (!additive && render.depthWrite()) return SingleQuadParticle.Layer.TRANSLUCENT;
        String name = render.blend() + (render.depthWrite() ? "" : NO_DEPTH);
        return MADE.computeIfAbsent(name, n -> new SingleQuadParticle.Layer(true,
                SingleQuadParticle.Layer.TRANSLUCENT.textureAtlasLocation(), pipeline(n, additive, render.depthWrite())));
    }

    /**
     * The game's translucent particles' pipeline, blending {@code additive}ly
     * (its light added by its alpha, as Hex's conjured particles) or not, and
     * writing depth or not.
     */
    private static RenderPipeline pipeline(String name, boolean additive, boolean depthWrite) {
        RenderPipeline base = RenderPipelines.TRANSLUCENT_PARTICLE;
        RenderPipeline.Builder b = RenderPipeline.builder()
                .withLocation(Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, PIPELINE_PATH + name))
                .withVertexShader(base.getVertexShader())
                .withFragmentShader(base.getFragmentShader())
                .withPolygonMode(base.getPolygonMode())
                .withCull(base.isCull())
                .withPrimitiveTopology(base.getPrimitiveTopology())
                .withColorTargetState(new ColorTargetState(additive ? BlendFunction.LIGHTNING : BlendFunction.TRANSLUCENT));
        DepthStencilState depth = base.getDepthStencilState();
        if (depth != null) b.withDepthStencilState(new DepthStencilState(depth.depthTest(), depthWrite));
        VertexFormat[] formats = base.getVertexFormatBindings();
        for (int i = 0; i < formats.length; i++) b.withVertexBinding(i, formats[i]);
        base.getBindGroupLayouts().forEach(b::withBindGroupLayout);
        ShaderDefines defines = base.getShaderDefines();
        defines.flags().forEach(b::withShaderDefine);
        defines.values().forEach((k, v) -> define(b, k, v));
        return b.build();
    }

    /** Define {@code k} as {@code v}: a whole number, else a decimal one. */
    private static void define(RenderPipeline.Builder b, String k, String v) {
        try {
            b.withShaderDefine(k, Integer.parseInt(v));
        } catch (NumberFormatException notWhole) {
            b.withShaderDefine(k, Float.parseFloat(v));
        }
    }
}
