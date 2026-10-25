package dev.yog.body;

import net.minecraft.core.Registry;
import dev.yog.NativeBridge;

import dev.yog.bus.YogTypes;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Vector3f;
import dev.yog.mixin.ModelPartAccess;
import dev.yog.util.YogBodyWire;
import dev.yog.util.YogBodyWire.Captured;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.WeakHashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.EntityModel;
import net.minecraft.client.model.HierarchicalModel;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.model.PlayerModel;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.LightTexture;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.RenderType;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.ItemRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.util.Mth;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MoverType;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.Vec3;
import com.mojang.math.Matrix3f;
import com.mojang.math.Matrix4f;

/**
 * yog-body on this host (see {@code docs/design/yog-body.md}): asks the mods
 * about tracked entities' bodies while they're drawn, about the camera and
 * about the local player's travel step, and does what they say.
 */
public final class YogBody {
    private YogBody() {
    }

    // ── Bodies ────────────────────────────────────────────────────────────────

    /** One entity being drawn: what the mods said, the parts changed. */
    private static final class State {
        static final State NONE = new State(dev.yog.util.YogBodyBus.NONE);
        final YogTypes.BodyOps ops;
        final List<Saved> saved = new ArrayList<>();

        State(YogTypes.BodyOps ops) {
            this.ops = ops;
        }
    }

    /** A part as vanilla posed it, to put back after drawing. */
    private record Saved(ModelPart p, float x, float y, float z, float xRot, float yRot, float zRot,
                         float xScale, float yScale, float zScale, boolean visible) {
        Saved(ModelPart p) {
            this(p, p.x, p.y, p.z, p.xRot, p.yRot, p.zRot, p.xScale, p.yScale, p.zScale, p.visible);
        }

        void restore() {
            p.x = x; p.y = y; p.z = z;
            p.xRot = xRot; p.yRot = yRot; p.zRot = zRot;
            p.xScale = xScale; p.yScale = yScale; p.zScale = zScale;
            p.visible = visible;
        }
    }

    /** Entities being drawn (a disguise draws another inside one). */
    private static final Deque<State> DRAWING = new ArrayDeque<>();
    /** Drawing a capture or a stand-in: the mods aren't asked. */
    private static int capturing;

    private static String typeId(Entity e) {
        return Registry.ENTITY_TYPE.getKey(e.getType()).toString();
    }

    private static float bodyYaw(LivingEntity e, float partial) {
        return Mth.rotLerp(partial, e.yBodyRotO, e.yBodyRot);
    }

    /** Render HEAD: asks the mods; true if the entity itself isn't drawn. */
    public static boolean begin(LivingEntityRenderer<?, ?> self, LivingEntity e, float yaw, float partial, Into into) {
        PoseStack pose = into.pose();
        MultiBufferSource buffers = into.buffers();
        int light = into.light();
        if (capturing > 0 || !YogBodyWire.anyTracked() || !YogBodyWire.tracked(e.getUUID(), typeId(e))) {
            DRAWING.push(State.NONE);
            return false;
        }
        float body = bodyYaw(e, partial);
        float head = Mth.rotLerp(partial, e.yHeadRotO, e.yHeadRot);
        boolean alive = e.isAlive() && !e.isPassenger();
        Vec3 v = e.getDeltaMovement();
        YogTypes.BodyFrame frame = YogBodyWire.frame(new YogBodyWire.Pose(e.getUUID(), typeId(e), partial, e.tickCount + partial,
                alive ? e.walkAnimation.position(partial) : 0, alive ? Math.min(1, e.walkAnimation.speed(partial)) : 0,
                head - body, Mth.lerp(partial, e.xRotO, e.getXRot()), body, e.getXRot(),
                Mth.lerp(partial, e.xo, e.getX()), Mth.lerp(partial, e.yo, e.getY()), Mth.lerp(partial, e.zo, e.getZ()),
                (float) v.x, (float) v.y, (float) v.z, NativeBridge.entityFlags(e), e.getLevel().getGameTime() + (double) partial));
        YogTypes.BodyOps ops = dev.yog.util.YogBodyBus.bodyOps(frame);
        if (!ops.captures().isEmpty()) capture(self, e, ops.captures(), partial, light);
        if (ops.disguise() != null) {
            Entity d = dummy(e, ops.disguise());
            if (d != null) {
                capturing++; // the stand-in isn't asked about
                try {
                    Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(d).render(d, yaw, partial, pose, buffers, light);
                } finally {
                    capturing--;
                }
            }
        }
        if (ops.hide()) {
            if (YogBodyWire.has(ops, YogTypes.BodyAnchor.ENTITY)) {
                pose.pushPose();
                pose.mulPose(Vector3f.YP.rotationDegrees(180.0F - body));
                if (ops.root() != null) pose.mulPoseMatrix(matrix(ops.root()));
                draw(ops, YogTypes.BodyAnchor.ENTITY, null, new Into(pose, buffers, light));
                pose.popPose();
            }
            return true;
        }
        DRAWING.push(new State(ops));
        return false;
    }

    /** After vanilla turned the body: the mods' turn, then their entity-space meshes. */
    public static void rotated(PoseStack pose, MultiBufferSource buffers, int light) {
        State s = DRAWING.peek();
        if (s == null || s.ops == dev.yog.util.YogBodyBus.NONE) return;
        if (s.ops.root() != null) pose.mulPoseMatrix(matrix(s.ops.root()));
        draw(s.ops, YogTypes.BodyAnchor.ENTITY, null, new Into(pose, buffers, light));
    }

    /** After vanilla animated the model: the mods' part changes. */
    public static void posed(EntityModel<?> model) {
        State s = DRAWING.peek();
        if (s == null || s.ops.bones().isEmpty()) return;
        Map<String, ModelPart[]> parts = parts(model);
        for (YogTypes.BodyBone bone : s.ops.bones()) {
            YogTypes.BodyBoneOp o = bone.op();
            ModelPart[] chain = parts.get(bone.name());
            if (chain == null) continue;
            ModelPart p = chain[chain.length - 1];
            s.saved.add(new Saved(p));
            if (o.setRotation()) {
                p.xRot = o.rotation().x(); p.yRot = o.rotation().y(); p.zRot = o.rotation().z();
            } else {
                p.xRot += o.rotation().x(); p.yRot += o.rotation().y(); p.zRot += o.rotation().z();
            }
            if (o.setPosition()) {
                p.x = o.position().x(); p.y = o.position().y(); p.z = o.position().z();
            } else {
                p.x += o.position().x(); p.y += o.position().y(); p.z += o.position().z();
            }
            if (o.scale() != null) {
                p.xScale *= o.scale().x(); p.yScale *= o.scale().y(); p.zScale *= o.scale().z();
            }
            if (Boolean.FALSE.equals(o.visible())) p.visible = false;
            if (Boolean.TRUE.equals(o.visible())) p.visible = true;
        }
    }

    /** After the model and its layers: the mods' model- and part-space meshes. */
    public static void drawn(EntityModel<?> model, PoseStack pose, MultiBufferSource buffers, int light) {
        State s = DRAWING.peek();
        if (s == null || s.ops.draws().isEmpty()) return;
        draw(s.ops, YogTypes.BodyAnchor.MODEL, null, new Into(pose, buffers, light));
        if (YogBodyWire.has(s.ops, YogTypes.BodyAnchor.BONE)) draw(s.ops, YogTypes.BodyAnchor.BONE, parts(model), new Into(pose, buffers, light));
    }

    /** Render RETURN: the parts back as vanilla left them. */
    public static void end() {
        State s = DRAWING.poll();
        if (s == null) return;
        for (int i = s.saved.size() - 1; i >= 0; i--) s.saved.get(i).restore();
    }

    private static void draw(YogTypes.BodyOps ops, String anchor, Map<String, ModelPart[]> parts, Into into) {
        PoseStack pose = into.pose();
        MultiBufferSource buffers = into.buffers();
        int light = into.light();
        for (YogTypes.BodyDraw m : ops.draws()) {
            if (!m.anchor().equals(anchor)) continue;
            pose.pushPose();
            if (anchor.equals(YogTypes.BodyAnchor.BONE)) {
                ModelPart[] chain = parts.get(m.bone());
                if (chain == null) {
                    pose.popPose();
                    continue;
                }
                for (ModelPart p : chain) p.translateAndRotate(pose);
            }
            if (!anchor.equals(YogTypes.BodyAnchor.ENTITY)) pose.scale(1 / 16.0F, 1 / 16.0F, 1 / 16.0F);
            emit(m, pose, buffers, light);
            pose.popPose();
        }
    }

    private static void emit(YogTypes.BodyDraw m, PoseStack pose, MultiBufferSource buffers, int light) {
        ResourceLocation tex = ResourceLocation.tryParse(m.mesh().texture());
        if (tex == null) return;
        RenderType type = switch (m.mode()) {
            case YogTypes.BodyMode.TRANSLUCENT -> RenderType.entityTranslucent(tex);
            case YogTypes.BodyMode.EMISSIVE -> RenderType.eyes(tex);
            default -> RenderType.entityCutoutNoCull(tex);
        };
        VertexConsumer vc = m.glint() ? ItemRenderer.getArmorFoilBuffer(buffers, type, false, true) : buffers.getBuffer(type);
        int lit = YogTypes.BodyMode.EMISSIVE.equals(m.mode()) ? LightTexture.FULL_BRIGHT : light;
        int argb = (int) m.tint();
        int a = argb >>> 24, r = argb >> 16 & 0xFF, g = argb >> 8 & 0xFF, b = argb & 0xFF;
        Matrix4f mat = pose.last().pose();
        Matrix3f nm = pose.last().normal();
        float[] v = m.mesh().verts();
        for (int i = 0; i + 8 <= v.length; i += 8) {
            vc.vertex(mat, v[i], v[i + 1], v[i + 2]).color(r, g, b, a).uv(v[i + 3], v[i + 4])
                    .overlayCoords(overlay).uv2(lit).normal(nm, v[i + 5], v[i + 6], v[i + 7]).endVertex();
        }
    }

    /** The matrix of its 16 floats, column by column. */
    private static Matrix4f matrix(float[] columns) {
        Matrix4f m = new Matrix4f();
        m.load(java.nio.FloatBuffer.wrap(columns));
        return m;
    }

    /** The overlay meshes are drawn with: a mod entity's hurt flash, else none. */
    private static int overlay = OverlayTexture.NO_OVERLAY;

    /**
     * A mod's own entity ({@link YogEntityRenderer}): no vanilla model, only
     * what the mods draw — entity space after its yaw, then model space.
     */
    public static void drawEntity(Entity e, float partial, PoseStack pose, MultiBufferSource buffers, int light) {
        if (capturing > 0 || !YogBodyWire.tracked(e.getUUID(), typeId(e))) return;
        LivingEntity living = e instanceof LivingEntity l ? l : null;
        float body = living != null ? bodyYaw(living, partial) : Mth.rotLerp(partial, e.yRotO, e.getYRot());
        float head = living != null ? Mth.rotLerp(partial, living.yHeadRotO, living.yHeadRot) : body;
        boolean walks = living != null && living.isAlive() && !living.isPassenger();
        Vec3 v = e.getDeltaMovement();
        YogTypes.BodyFrame frame = YogBodyWire.frame(new YogBodyWire.Pose(e.getUUID(), typeId(e), partial, e.tickCount + partial,
                walks ? living.walkAnimation.position(partial) : 0, walks ? Math.min(1, living.walkAnimation.speed(partial)) : 0,
                head - body, Mth.lerp(partial, e.xRotO, e.getXRot()), body, e.getXRot(),
                Mth.lerp(partial, e.xo, e.getX()), Mth.lerp(partial, e.yo, e.getY()), Mth.lerp(partial, e.zo, e.getZ()),
                (float) v.x, (float) v.y, (float) v.z, NativeBridge.entityFlags(e), e.getLevel().getGameTime() + (double) partial));
        YogTypes.BodyOps ops = dev.yog.util.YogBodyBus.bodyOps(frame);
        if (ops == dev.yog.util.YogBodyBus.NONE) return;
        overlay = living != null ? LivingEntityRenderer.getOverlayCoords(living, 0) : OverlayTexture.NO_OVERLAY;
        try {
            pose.pushPose();
            pose.mulPose(Vector3f.YP.rotationDegrees(180.0F - body));
            if (ops.root() != null) pose.mulPoseMatrix(matrix(ops.root()));
            draw(ops, YogTypes.BodyAnchor.ENTITY, null, new Into(pose, buffers, light));
            pose.scale(-1.0F, -1.0F, 1.0F);
            pose.translate(0.0F, -1.501F, 0.0F);
            draw(ops, YogTypes.BodyAnchor.MODEL, null, new Into(pose, buffers, light));
            pose.popPose();
        } finally {
            overlay = OverlayTexture.NO_OVERLAY;
        }
    }

    // ── Parts by name ─────────────────────────────────────────────────────────

    private static final Map<EntityModel<?>, Map<String, ModelPart[]>> PARTS = new WeakHashMap<>();

    /** A model's parts by name, each with the parts it hangs on (outermost first). */
    private static Map<String, ModelPart[]> parts(EntityModel<?> model) {
        return PARTS.computeIfAbsent(model, m -> {
            Map<String, ModelPart[]> out = new LinkedHashMap<>();
            if (m instanceof HierarchicalModel<?> h) walk(h.root(), "root", new ModelPart[0], out);
            if (m instanceof HumanoidModel<?> h) {
                out.put("head", new ModelPart[] {h.head});
                out.put("hat", new ModelPart[] {h.hat});
                out.put("body", new ModelPart[] {h.body});
                out.put("right_arm", new ModelPart[] {h.rightArm});
                out.put("left_arm", new ModelPart[] {h.leftArm});
                out.put("right_leg", new ModelPart[] {h.rightLeg});
                out.put("left_leg", new ModelPart[] {h.leftLeg});
            }
            if (m instanceof PlayerModel<?> p) {
                out.put("jacket", new ModelPart[] {p.jacket});
                out.put("left_sleeve", new ModelPart[] {p.leftSleeve});
                out.put("right_sleeve", new ModelPart[] {p.rightSleeve});
                out.put("left_pants", new ModelPart[] {p.leftPants});
                out.put("right_pants", new ModelPart[] {p.rightPants});
            }
            return out;
        });
    }

    private static void walk(ModelPart part, String name, ModelPart[] above, Map<String, ModelPart[]> out) {
        ModelPart[] chain = java.util.Arrays.copyOf(above, above.length + 1);
        chain[above.length] = part;
        out.putIfAbsent(name, chain);
        for (Map.Entry<String, ModelPart> c : ((ModelPartAccess) (Object) part).yog$children().entrySet()) {
            walk(c.getValue(), c.getKey(), chain, out);
        }
    }

    // ── Disguises ─────────────────────────────────────────────────────────────

    /** Each disguised entity's stand-in. */
    private static final Map<UUID, Entity> DUMMIES = new HashMap<>();

    /** A stand-in of type {@code type} posed as {@code e} is; null if there's no such type. */
    private static Entity dummy(LivingEntity e, String type) {
        Entity d = DUMMIES.get(e.getUUID());
        if (d == null || d.getLevel() != e.getLevel() || !typeId(d).equals(type)) {
            d = EntityType.byString(type).map(t -> t.create(e.getLevel())).orElse(null);
            if (d == null) return null;
            d.setSilent(true);
            DUMMIES.put(e.getUUID(), d);
        }
        d.setPos(e.getX(), e.getY(), e.getZ());
        d.xo = e.xo; d.yo = e.yo; d.zo = e.zo;
        d.xOld = e.xOld; d.yOld = e.yOld; d.zOld = e.zOld;
        d.setYRot(e.getYRot()); d.yRotO = e.yRotO;
        d.setXRot(e.getXRot()); d.xRotO = e.xRotO;
        d.tickCount = e.tickCount;
        d.setOnGround(e.isOnGround());
        d.setPose(e.getPose());
        d.setShiftKeyDown(e.isShiftKeyDown());
        d.setSprinting(e.isSprinting());
        if (d instanceof LivingEntity l) {
            l.yBodyRot = e.yBodyRot; l.yBodyRotO = e.yBodyRotO;
            l.yHeadRot = e.yHeadRot; l.yHeadRotO = e.yHeadRotO;
            l.animationSpeedOld = e.animationSpeedOld;
            l.animationSpeed = e.animationSpeed;
            l.animationPosition = e.animationPosition;
            l.attackAnim = e.attackAnim; l.oAttackAnim = e.oAttackAnim;
            l.swinging = e.swinging; l.swingTime = e.swingTime;
            l.hurtTime = e.hurtTime; l.deathTime = e.deathTime;
            for (EquipmentSlot slot : EquipmentSlot.values()) {
                ItemStack want = e.getItemBySlot(slot);
                if (!ItemStack.matches(l.getItemBySlot(slot), want)) l.setItemSlot(slot, want.copy());
            }
        }
        return d;
    }

    // ── Captures ──────────────────────────────────────────────────────────────

    private static void capture(LivingEntityRenderer<?, ?> self, LivingEntity e, List<String> sources, float partial, int light) {
        List<Captured> out = new ArrayList<>();
        float body = bodyYaw(e, partial);
        capturing++;
        try {
            for (String source : sources) {
                Entity target = source.equals("self") ? e : dummy(e, source);
                if (target == null) continue;
                PoseStack pose = new PoseStack();
                pose.mulPose(Vector3f.YP.rotationDegrees(body - 180.0F));
                Buffers buffers = new Buffers();
                @SuppressWarnings("unchecked") // an entity's renderer and model are of its own type: the game hands them out untyped
                EntityRenderer<Entity> r = target == e
                        ? (EntityRenderer<Entity>) (Object) self
                        : (EntityRenderer<Entity>) Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(target);
                r.render(target, 0, partial, pose, buffers, light);
                Captured c = new Captured(source);
                buffers.sinks.forEach((tex, sink) -> c.meshes.put(tex, sink.quads()));
                out.add(c);
            }
        } finally {
            capturing--;
        }
        YogBodyWire.captured(e.getUUID(), out);
    }

    /** Buffers that keep what's drawn into them, by texture. */
    private static final class Buffers implements MultiBufferSource {
        final Map<String, YogBodyWire.Sink> sinks = new LinkedHashMap<>();
        private static final YogBodyWire.Sink NOWHERE = new YogBodyWire.Sink();

        @Override
        public VertexConsumer getBuffer(RenderType type) {
            String tex = YogBodyWire.textureOf(type);
            return new Keep(tex == null ? NOWHERE : sinks.computeIfAbsent(tex, t -> new YogBodyWire.Sink()));
        }
    }

    private record Keep(YogBodyWire.Sink sink) implements VertexConsumer {
        @Override
        public VertexConsumer vertex(double x, double y, double z) {
            sink.vertex((float) x, (float) y, (float) z);
            return this;
        }

        @Override
        public VertexConsumer color(int r, int g, int b, int a) {
            return this;
        }

        @Override
        public VertexConsumer uv(float u, float v) {
            sink.uv(u, v);
            return this;
        }

        @Override
        public VertexConsumer overlayCoords(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer uv2(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer normal(float x, float y, float z) {
            sink.normal(x, y, z);
            return this;
        }

        @Override
        public void endVertex() {
            sink.end();
        }

        @Override
        public void defaultColor(int r, int g, int b, int a) {
        }

        @Override
        public void unsetDefaultColor() {
        }
    }

    // ── Camera ────────────────────────────────────────────────────────────────

    /** The field of view the mods scale the game's by (ViewMixin); the roll goes in the camera's rotation (CameraMixin). */
    public static float fov = 1;

    /** Where a body is drawn: its pose stack, what takes its quads, the light. */
    public record Into(PoseStack pose, MultiBufferSource buffers, int light) {}

    /** Which way a camera looks. */
    public record Look(float yaw, float pitch) {}

    /**
     * Camera.setup TAIL: {right, up, forward, wx, wy, wz, yaw, pitch, roll,
     * fov} from the mods, or null.
     */
    public static float[] camera(Entity entity, float partial, Vec3 pos, Look look, boolean detached) {
        float[] o = entity == null ? null : dev.yog.util.YogBodyBus.camera(new dev.yog.bus.YogTypes.CameraFrame(entity.getStringUUID(), partial,
                new double[] {pos.x, pos.y, pos.z}, look.yaw(), look.pitch(), detached, NativeBridge.entityFlags(entity)));
        fov = o == null ? 1 : o[9];
        return o;
    }

    // ── Movement ──────────────────────────────────────────────────────────────

    /** LivingEntity.travel HEAD: true if a mod moved the local player instead. */
    public static boolean travel(LivingEntity self) {
        if (!self.getLevel().isClientSide || !(self instanceof LocalPlayer p) || p != Minecraft.getInstance().player) return false;
        Vec3 v = p.getDeltaMovement();
        Vec3 look = p.getLookAngle();
        float gravity = v.y <= 0 && p.hasEffect(MobEffects.SLOW_FALLING) ? 0.01F : 0.08F;
        float[] out = dev.yog.util.YogBodyBus.travel(new dev.yog.bus.YogTypes.TravelFrame(p.getStringUUID(),
                new dev.yog.bus.YogTypes.Vec3((float) v.x, (float) v.y, (float) v.z), new dev.yog.bus.YogTypes.Vec3((float) look.x, (float) look.y, (float) look.z),
                p.getYRot(), p.getXRot(), p.zza, p.xxa, p.input.jumping, p.isShiftKeyDown(), gravity, NativeBridge.entityFlags(p)));
        if (out == null) return false;
        Vec3 move = new Vec3(out[0], out[1], out[2]);
        p.setDeltaMovement(move);
        p.move(MoverType.SELF, move);
        p.calculateEntityAnimation(false);
        return true;
    }
}
