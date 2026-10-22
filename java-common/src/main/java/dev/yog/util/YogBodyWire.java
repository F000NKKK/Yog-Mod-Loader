package dev.yog.util;

import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * yog-body (see {@code docs/design/yog-body.md}) as far as it doesn't depend on
 * the game's classes: which entities the mods ask about, the frame the host
 * sends ({@link YogTypes.BodyFrame}), what the mods' ops draw, captured geometry.
 */
public final class YogBodyWire {
    private YogBodyWire() {
    }

    // ── Tracking ──────────────────────────────────────────────────────────────

    private static final Set<UUID> TRACKED = ConcurrentHashMap.newKeySet();
    private static final Set<String> TRACKED_TYPES = ConcurrentHashMap.newKeySet();

    public static void track(String uuid, boolean on) {
        try {
            UUID id = UUID.fromString(uuid);
            if (on) TRACKED.add(id); else TRACKED.remove(id);
        } catch (IllegalArgumentException e) {
            dev.yog.YogLog.warn("body tracking: " + uuid + " isn't a UUID");
        }
    }

    public static void trackType(String type, boolean on) {
        if (on) TRACKED_TYPES.add(type); else TRACKED_TYPES.remove(type);
    }

    /** Whether the mods are asked about this entity. */
    public static boolean tracked(UUID id, String type) {
        return (!TRACKED.isEmpty() && TRACKED.contains(id)) || (!TRACKED_TYPES.isEmpty() && TRACKED_TYPES.contains(type));
    }

    public static boolean anyTracked() {
        return !TRACKED.isEmpty() || !TRACKED_TYPES.isEmpty();
    }

    // ── Ops ──────────────────────────────────────────────────────────────────

    /** Whether the mods draw anything anchored at {@code anchor} ({@link YogTypes.BodyAnchor}). */
    public static boolean has(YogTypes.BodyOps ops, String anchor) {
        for (YogTypes.BodyDraw d : ops.draws()) if (d.anchor().equals(anchor)) return true;
        return false;
    }

    // ── Captures ──────────────────────────────────────────────────────────────

    /** The posed geometry of a body, by texture (entity space). */
    public static final class Captured {
        public final String source;
        public final Map<String, float[]> meshes = new LinkedHashMap<>();

        public Captured(String source) {
            this.source = source;
        }
    }

    /** Captures for each entity, sent with its next frame. */
    private static final Map<UUID, List<Captured>> CAPTURED = new ConcurrentHashMap<>();

    public static void captured(UUID id, List<Captured> captures) {
        if (captures.isEmpty()) CAPTURED.remove(id); else CAPTURED.put(id, captures);
    }

    /**
     * A render type's texture in its description: {@code texture[Optional[id]]}
     * before 26.1, a {@code TextureBinding[location=id, ...]} in 26.x.
     */
    private static final Pattern TEXTURE = Pattern.compile("(?:texture\\[Optional\\[|TextureBinding\\[location=)([^\\],]+)");

    /** The texture a render type draws with, from its description; null for none. */
    public static String textureOf(Object renderType) {
        Matcher m = TEXTURE.matcher(String.valueOf(renderType));
        if (!m.find()) return null;
        String t = m.group(1);
        return t.contains("textures/") ? t : null;
    }

    /** Collects vertices drawn into one buffer: 8 floats each. */
    public static final class Sink {
        private float[] data = new float[256];
        private int size;
        private final float[] cur = new float[8];
        private boolean open;

        /** Starts a vertex at x, y, z (ends the one before). */
        public void vertex(float x, float y, float z) {
            end();
            Arrays.fill(cur, 0);
            cur[0] = x;
            cur[1] = y;
            cur[2] = z;
            open = true;
        }

        public void uv(float u, float v) {
            cur[3] = u;
            cur[4] = v;
        }

        public void normal(float x, float y, float z) {
            cur[5] = x;
            cur[6] = y;
            cur[7] = z;
        }

        /** Ends the vertex being made. */
        public void end() {
            if (!open) return;
            if (size + 8 > data.length) data = Arrays.copyOf(data, data.length * 2);
            System.arraycopy(cur, 0, data, size, 8);
            size += 8;
            open = false;
        }

        /** The whole quads made. */
        public float[] quads() {
            end();
            int n = size / 32 * 32;
            return Arrays.copyOf(data, n);
        }
    }

    // ── Frame ─────────────────────────────────────────────────────────────────

    /** Floats a captured vertex takes: x y z, u v, nx ny nz. */
    private static final int VERTEX_FLOATS = 8;

    /** An entity as a frame tells it: who, how far into the tick, its limbs, look, place, motion, flags, the game's time. */
    public record Pose(UUID id, String type, float partial, float age, float limbSwing, float limbAmount,
                       float headYaw, float headPitch, float bodyYaw, float pitch,
                       double x, double y, double z, float vx, float vy, float vz, int flags, double gameTime) {
    }

    /**
     * The frame the mods read ({@code yog_body::Frame}), with the entity's
     * last captures; the runtime adds the data it keeps.
     */
    public static YogTypes.BodyFrame frame(Pose p) {
        List<Captured> caps = CAPTURED.remove(p.id());
        List<YogTypes.BodyCapture> captures = new ArrayList<>();
        if (caps != null) {
            for (Captured c : caps) {
                List<YogTypes.BodyMesh> meshes = new ArrayList<>(c.meshes.size());
                for (Map.Entry<String, float[]> m : c.meshes.entrySet()) {
                    float[] v = m.getValue();
                    meshes.add(new YogTypes.BodyMesh(m.getKey(), Arrays.copyOf(v, v.length / VERTEX_FLOATS * VERTEX_FLOATS)));
                }
                captures.add(new YogTypes.BodyCapture(c.source, meshes));
            }
        }
        return new YogTypes.BodyFrame(p.id().toString(), p.type(), p.partial(), p.age(), p.limbSwing(), p.limbAmount(),
                p.headYaw(), p.headPitch(), p.bodyYaw(), p.pitch(), new double[] {p.x(), p.y(), p.z()},
                new YogTypes.Vec3(p.vx(), p.vy(), p.vz()), Integer.toUnsignedLong(p.flags()), p.gameTime(), captures, List.of());
    }
}
