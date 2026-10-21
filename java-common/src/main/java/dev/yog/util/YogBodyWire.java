package dev.yog.util;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
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
 * yog-body's wire (see {@code docs/design/yog-body.md}) and what doesn't
 * depend on the game's classes: which entities the mods ask about, the frame
 * the host sends, the ops a mod answers with, captured geometry.
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
        } catch (IllegalArgumentException ignored) {
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

    // ── Ops ───────────────────────────────────────────────────────────────────

    public static final int ANCHOR_ENTITY = 0, ANCHOR_MODEL = 1, ANCHOR_BONE = 2, ANCHOR_BLOCK = 3;
    public static final int MODE_CUTOUT = 0, MODE_TRANSLUCENT = 1, MODE_EMISSIVE = 2;
    public static final int BONE_SET_ROT = 1, BONE_SET_POS = 2, BONE_SCALE = 4, BONE_HIDE = 8, BONE_SHOW = 16;

    /** Quads with one texture: x y z u v nx ny nz per vertex, 4 vertices a quad. */
    public static final class Mesh {
        public int anchor;
        public String bone;
        public String texture;
        public int mode;
        public boolean glint;
        public int argb;
        public float[] verts;
    }

    /** A vanilla part's change. */
    public static final class BoneOp {
        public String name;
        public int flags;
        public float rx, ry, rz, px, py, pz, sx, sy, sz;
    }

    /** What the mods said about one entity this frame. */
    public static final class Ops {
        public static final Ops NONE = new Ops();

        /** Column-major, entity space; null for none. */
        public float[] root;
        public final List<BoneOp> bones = new ArrayList<>();
        public final List<Mesh> meshes = new ArrayList<>();
        public String disguise;
        public boolean hide;
        public final List<String> captures = new ArrayList<>();

        public boolean has(int anchor) {
            for (Mesh m : meshes) if (m.anchor == anchor) return true;
            return false;
        }

        /** Reads the ops; what can't be read ends them. */
        public static Ops parse(byte[] bytes) {
            if (bytes == null || bytes.length == 0) return NONE;
            Ops ops = new Ops();
            ByteBuffer b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
            try {
                while (b.hasRemaining()) {
                    switch (b.get()) {
                        case 1 -> {
                            float[] m = new float[16];
                            for (int i = 0; i < 16; i++) m[i] = b.getFloat();
                            ops.root = ops.root == null ? m : multiply(ops.root, m);
                        }
                        case 2 -> {
                            BoneOp o = new BoneOp();
                            o.name = str(b);
                            o.flags = b.get() & 0xFF;
                            o.rx = b.getFloat(); o.ry = b.getFloat(); o.rz = b.getFloat();
                            o.px = b.getFloat(); o.py = b.getFloat(); o.pz = b.getFloat();
                            o.sx = b.getFloat(); o.sy = b.getFloat(); o.sz = b.getFloat();
                            ops.bones.add(o);
                        }
                        case 3 -> {
                            Mesh m = new Mesh();
                            m.anchor = b.get() & 0xFF;
                            m.bone = str(b);
                            m.texture = str(b);
                            m.mode = b.get() & 0xFF;
                            m.glint = b.get() != 0;
                            m.argb = b.getInt();
                            int n = b.getInt();
                            m.verts = new float[n * 8];
                            for (int i = 0; i < m.verts.length; i++) m.verts[i] = b.getFloat();
                            ops.meshes.add(m);
                        }
                        case 4 -> ops.disguise = str(b);
                        case 5 -> ops.hide = true;
                        case 6 -> ops.captures.add(str(b));
                        default -> {
                            return ops;
                        }
                    }
                }
            } catch (RuntimeException ignored) {
                // a short record: what came before it stands
            }
            return ops;
        }
    }

    /** {@code a × b}, both column-major 4×4. */
    public static float[] multiply(float[] a, float[] b) {
        float[] r = new float[16];
        for (int c = 0; c < 4; c++) {
            for (int row = 0; row < 4; row++) {
                float s = 0;
                for (int k = 0; k < 4; k++) s += a[k * 4 + row] * b[c * 4 + k];
                r[c * 4 + row] = s;
            }
        }
        return r;
    }

    private static String str(ByteBuffer b) {
        int n = b.getShort() & 0xFFFF;
        byte[] s = new byte[n];
        b.get(s);
        return new String(s, StandardCharsets.UTF_8);
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

    /** A growing little-endian buffer. */
    private static final class Out {
        private ByteBuffer b = ByteBuffer.allocate(256).order(ByteOrder.LITTLE_ENDIAN);

        private void room(int n) {
            if (b.remaining() >= n) return;
            ByteBuffer bigger = ByteBuffer.allocate(Math.max(b.capacity() * 2, b.position() + n)).order(ByteOrder.LITTLE_ENDIAN);
            b.flip();
            bigger.put(b);
            b = bigger;
        }

        Out u8(int v) { room(1); b.put((byte) v); return this; }
        Out u16(int v) { room(2); b.putShort((short) v); return this; }
        Out u32(int v) { room(4); b.putInt(v); return this; }
        Out f32(float v) { room(4); b.putFloat(v); return this; }
        Out f64(double v) { room(8); b.putDouble(v); return this; }

        Out str(String s) {
            byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
            int n = Math.min(bytes.length, 0xFFFF);
            room(2 + n);
            b.putShort((short) n);
            b.put(bytes, 0, n);
            return this;
        }

        Out floats(float[] f) {
            room(f.length * 4);
            for (float v : f) b.putFloat(v);
            return this;
        }

        byte[] bytes() {
            return Arrays.copyOf(b.array(), b.position());
        }
    }

    /** What starts a frame: its kind, {@code yog_body::wire}'s. */
    private static final int FRAME_TAG = 2;

    /** An entity as a frame tells it: who, how far into the tick, its limbs, look, place, motion, flags, the game's time. */
    public record Pose(UUID id, String type, float partial, float age, float limbSwing, float limbAmount,
                       float headYaw, float headPitch, float bodyYaw, float pitch,
                       double x, double y, double z, float vx, float vy, float vz, int flags, double gameTime) {
    }

    /**
     * The frame the mods read ({@code yog_body::Frame}), with the entity's
     * last captures; the runtime adds the data it keeps.
     */
    public static byte[] frame(Pose p) {
        Out o = new Out().u8(FRAME_TAG).str(p.id().toString()).str(p.type());
        o.f32(p.partial()).f32(p.age()).f32(p.limbSwing()).f32(p.limbAmount()).f32(p.headYaw()).f32(p.headPitch()).f32(p.bodyYaw()).f32(p.pitch());
        o.f64(p.x()).f64(p.y()).f64(p.z()).f32(p.vx()).f32(p.vy()).f32(p.vz()).u32(p.flags()).f64(p.gameTime());
        List<Captured> caps = CAPTURED.remove(p.id());
        if (caps == null) caps = List.of();
        o.u16(caps.size());
        for (Captured c : caps) {
            o.str(c.source).u16(c.meshes.size());
            for (Map.Entry<String, float[]> m : c.meshes.entrySet()) {
                o.str(m.getKey()).u32(m.getValue().length / 8).floats(m.getValue());
            }
        }
        return o.bytes();
    }
}
