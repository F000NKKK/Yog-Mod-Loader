package dev.yog.render;

import com.mojang.blaze3d.opengl.GlTexture;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.textures.GpuTexture;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.Minecraft;
import org.lwjgl.opengl.GL33C;

/**
 * Raw OpenGL from Rust on 26.x's render-pass backend (OpenGL only): the
 * game binds a framebuffer per pass and unbinds it after, and caches the
 * state it set (program, blend, depth, textures) to skip redundant calls.
 * {@link #begin} points GL at the main render target and remembers the
 * state; {@link #end} puts every bit of it back, so the game's caches are
 * true again.
 */
public final class YogGl {
    private YogGl() {}

    /** Framebuffers over the main target's color + depth textures, by the pair of texture ids. */
    private static final Map<Long, Integer> FBOS = new HashMap<>();
    /** The texture units whose binding is kept (the game's samplers use the first few). */
    private static final int UNITS = 4;
    private static final int[] SAVED_TEXTURES = new int[UNITS];
    private static final int[] SAVED_VIEWPORT = new int[4];
    private static final int[] SAVED_SCISSOR_BOX = new int[4];
    private static final boolean[] SAVED_COLOR_MASK = new boolean[4];
    private static int savedFbo, savedProgram, savedVao, savedArrayBuffer, savedActiveTexture;
    private static int savedBlendSrcRgb, savedBlendDstRgb, savedBlendSrcAlpha, savedBlendDstAlpha;
    private static int savedBlendEqRgb, savedBlendEqAlpha, savedDepthFunc;
    private static boolean savedBlend, savedDepthTest, savedCull, savedScissor, savedDepthMask;
    /** Whether {@link #begin} bound a target, so {@link #end} has something to restore. */
    private static boolean active;

    /** Remembers GL's state and binds the main render target (color + depth) for drawing. */
    public static boolean begin() {
        RenderTarget target = Minecraft.getInstance().gameRenderer.mainRenderTarget();
        return beginOn(target.getColorTexture(), target.getDepthTexture(), target.width, target.height);
    }

    /** Remembers GL's state and binds {@code color} and {@code depth} ({@code w}×{@code h}) for drawing. */
    static boolean beginOn(GpuTexture colorTexture, GpuTexture depthTexture, int w, int h) {
        if (!(colorTexture instanceof GlTexture color) || !(depthTexture instanceof GlTexture depth)) {
            return false;
        }
        save();
        GL33C.glBindFramebuffer(GL33C.GL_FRAMEBUFFER, fbo(color.glId(), depth.glId()));
        GL33C.glViewport(0, 0, w, h);
        GL33C.glDisable(GL33C.GL_SCISSOR_TEST);
        // The last pass may have left color writes masked off.
        GL33C.glColorMask(true, true, true, true);
        active = true;
        return true;
    }

    /** Puts back everything {@link #begin} remembered. */
    public static void end() {
        if (!active) return;
        active = false;
        restore();
    }

    private static int fbo(int color, int depth) {
        long key = ((long) color << Integer.SIZE) | (depth & 0xFFFF_FFFFL);
        Integer known = FBOS.get(key);
        if (known != null) return known;
        // A resize makes new textures: the framebuffer of the old ones goes.
        forget();
        int fbo = GL33C.glGenFramebuffers();
        GL33C.glBindFramebuffer(GL33C.GL_FRAMEBUFFER, fbo);
        GL33C.glFramebufferTexture2D(GL33C.GL_FRAMEBUFFER, GL33C.GL_COLOR_ATTACHMENT0, GL33C.GL_TEXTURE_2D, color, 0);
        GL33C.glFramebufferTexture2D(GL33C.GL_FRAMEBUFFER, GL33C.GL_DEPTH_ATTACHMENT, GL33C.GL_TEXTURE_2D, depth, 0);
        FBOS.put(key, fbo);
        return fbo;
    }

    private static void forget() {
        for (int fbo : FBOS.values()) GL33C.glDeleteFramebuffers(fbo);
        FBOS.clear();
    }

    private static void save() {
        savedFbo = GL33C.glGetInteger(GL33C.GL_DRAW_FRAMEBUFFER_BINDING);
        savedProgram = GL33C.glGetInteger(GL33C.GL_CURRENT_PROGRAM);
        savedVao = GL33C.glGetInteger(GL33C.GL_VERTEX_ARRAY_BINDING);
        savedArrayBuffer = GL33C.glGetInteger(GL33C.GL_ARRAY_BUFFER_BINDING);
        savedActiveTexture = GL33C.glGetInteger(GL33C.GL_ACTIVE_TEXTURE);
        for (int i = 0; i < UNITS; i++) {
            GL33C.glActiveTexture(GL33C.GL_TEXTURE0 + i);
            SAVED_TEXTURES[i] = GL33C.glGetInteger(GL33C.GL_TEXTURE_BINDING_2D);
        }
        GL33C.glActiveTexture(savedActiveTexture);
        GL33C.glGetIntegerv(GL33C.GL_VIEWPORT, SAVED_VIEWPORT);
        GL33C.glGetIntegerv(GL33C.GL_SCISSOR_BOX, SAVED_SCISSOR_BOX);
        savedBlend = GL33C.glIsEnabled(GL33C.GL_BLEND);
        savedDepthTest = GL33C.glIsEnabled(GL33C.GL_DEPTH_TEST);
        savedCull = GL33C.glIsEnabled(GL33C.GL_CULL_FACE);
        savedScissor = GL33C.glIsEnabled(GL33C.GL_SCISSOR_TEST);
        savedDepthMask = GL33C.glGetBoolean(GL33C.GL_DEPTH_WRITEMASK);
        savedDepthFunc = GL33C.glGetInteger(GL33C.GL_DEPTH_FUNC);
        savedBlendSrcRgb = GL33C.glGetInteger(GL33C.GL_BLEND_SRC_RGB);
        savedBlendDstRgb = GL33C.glGetInteger(GL33C.GL_BLEND_DST_RGB);
        savedBlendSrcAlpha = GL33C.glGetInteger(GL33C.GL_BLEND_SRC_ALPHA);
        savedBlendDstAlpha = GL33C.glGetInteger(GL33C.GL_BLEND_DST_ALPHA);
        savedBlendEqRgb = GL33C.glGetInteger(GL33C.GL_BLEND_EQUATION_RGB);
        savedBlendEqAlpha = GL33C.glGetInteger(GL33C.GL_BLEND_EQUATION_ALPHA);
        try (var stack = org.lwjgl.system.MemoryStack.stackPush()) {
            var mask = stack.malloc(SAVED_COLOR_MASK.length);
            GL33C.glGetBooleanv(GL33C.GL_COLOR_WRITEMASK, mask);
            for (int i = 0; i < SAVED_COLOR_MASK.length; i++) SAVED_COLOR_MASK[i] = mask.get(i) != 0;
        }
    }

    private static void restore() {
        GL33C.glBindFramebuffer(GL33C.GL_FRAMEBUFFER, savedFbo);
        GL33C.glUseProgram(savedProgram);
        GL33C.glBindVertexArray(savedVao);
        GL33C.glBindBuffer(GL33C.GL_ARRAY_BUFFER, savedArrayBuffer);
        for (int i = 0; i < UNITS; i++) {
            GL33C.glActiveTexture(GL33C.GL_TEXTURE0 + i);
            GL33C.glBindTexture(GL33C.GL_TEXTURE_2D, SAVED_TEXTURES[i]);
        }
        GL33C.glActiveTexture(savedActiveTexture);
        GL33C.glViewport(SAVED_VIEWPORT[0], SAVED_VIEWPORT[1], SAVED_VIEWPORT[2], SAVED_VIEWPORT[3]);
        GL33C.glScissor(SAVED_SCISSOR_BOX[0], SAVED_SCISSOR_BOX[1], SAVED_SCISSOR_BOX[2], SAVED_SCISSOR_BOX[3]);
        toggle(GL33C.GL_BLEND, savedBlend);
        toggle(GL33C.GL_DEPTH_TEST, savedDepthTest);
        toggle(GL33C.GL_CULL_FACE, savedCull);
        toggle(GL33C.GL_SCISSOR_TEST, savedScissor);
        GL33C.glDepthMask(savedDepthMask);
        GL33C.glDepthFunc(savedDepthFunc);
        GL33C.glBlendFuncSeparate(savedBlendSrcRgb, savedBlendDstRgb, savedBlendSrcAlpha, savedBlendDstAlpha);
        GL33C.glBlendEquationSeparate(savedBlendEqRgb, savedBlendEqAlpha);
        GL33C.glColorMask(SAVED_COLOR_MASK[0], SAVED_COLOR_MASK[1], SAVED_COLOR_MASK[2], SAVED_COLOR_MASK[3]);
    }

    private static void toggle(int cap, boolean on) {
        if (on) GL33C.glEnable(cap);
        else GL33C.glDisable(cap);
    }
}
