package dev.yog;

import dev.yog.mixin.LimbAnimatorAccess;
import dev.yog.mixin.ModelPartAccess;
import dev.yog.util.YogBodyWire;
import dev.yog.util.YogBodyWire.BoneOp;
import dev.yog.util.YogBodyWire.Captured;
import dev.yog.util.YogBodyWire.Mesh;
import dev.yog.util.YogBodyWire.Ops;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.WeakHashMap;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.client.render.entity.model.BipedEntityModel;
import net.minecraft.client.render.entity.model.EntityModel;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.render.entity.model.SinglePartEntityModel;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.MovementType;
import net.minecraft.entity.effect.StatusEffects;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.MathHelper;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix3f;
import org.joml.Matrix4f;

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
        static final State NONE = new State(Ops.NONE);
        final Ops ops;
        final List<Saved> saved = new ArrayList<>();

        State(Ops ops) {
            this.ops = ops;
        }
    }

    /** A part as vanilla posed it, to put back after drawing. */
    private record Saved(ModelPart p, float x, float y, float z, float pitch, float yaw, float roll,
                         float xScale, float yScale, float zScale, boolean visible) {
        Saved(ModelPart p) {
            this(p, p.pivotX, p.pivotY, p.pivotZ, p.pitch, p.yaw, p.roll, p.xScale, p.yScale, p.zScale, p.visible);
        }

        void restore() {
            p.pivotX = x; p.pivotY = y; p.pivotZ = z;
            p.pitch = pitch; p.yaw = yaw; p.roll = roll;
            p.xScale = xScale; p.yScale = yScale; p.zScale = zScale;
            p.visible = visible;
        }
    }

    /** Entities being drawn (a disguise draws another inside one). */
    private static final Deque<State> DRAWING = new ArrayDeque<>();
    /** Drawing a capture or a stand-in: the mods aren't asked. */
    private static int capturing;

    private static String typeId(Entity e) {
        return Registries.ENTITY_TYPE.getId(e.getType()).toString();
    }

    private static float bodyYaw(LivingEntity e, float partial) {
        return MathHelper.lerpAngleDegrees(partial, e.prevBodyYaw, e.bodyYaw);
    }

    /** Render HEAD: asks the mods; true if the entity itself isn't drawn. */
    public static boolean begin(LivingEntityRenderer<?, ?> self, LivingEntity e, float yaw, float partial,
                                MatrixStack pose, VertexConsumerProvider buffers, int light) {
        if (capturing > 0 || !YogBodyWire.anyTracked() || !YogBodyWire.tracked(e.getUuid(), typeId(e))) {
            DRAWING.push(State.NONE);
            return false;
        }
        float body = bodyYaw(e, partial);
        float head = MathHelper.lerpAngleDegrees(partial, e.prevHeadYaw, e.headYaw);
        boolean alive = e.isAlive() && !e.hasVehicle();
        Vec3d v = e.getVelocity();
        byte[] frame = YogBodyWire.frame(e.getUuid(), typeId(e), partial, e.age + partial,
                alive ? e.limbAnimator.getPos(partial) : 0, alive ? Math.min(1, e.limbAnimator.getSpeed(partial)) : 0,
                head - body, MathHelper.lerp(partial, e.prevPitch, e.getPitch()), body, e.getPitch(),
                MathHelper.lerp(partial, e.prevX, e.getX()), MathHelper.lerp(partial, e.prevY, e.getY()), MathHelper.lerp(partial, e.prevZ, e.getZ()),
                (float) v.x, (float) v.y, (float) v.z, NativeBridge.entityFlags(e), e.getWorld().getTime() + (double) partial);
        Ops ops = Ops.parse(NativeBridge.nativeBodyPose(frame));
        if (!ops.captures.isEmpty()) capture(self, e, ops.captures, partial, light);
        if (ops.disguise != null) {
            Entity d = dummy(e, ops.disguise);
            if (d != null) {
                capturing++; // the stand-in isn't asked about
                try {
                    MinecraftClient.getInstance().getEntityRenderDispatcher().getRenderer(d).render(d, yaw, partial, pose, buffers, light);
                } finally {
                    capturing--;
                }
            }
        }
        if (ops.hide) {
            if (ops.has(YogBodyWire.ANCHOR_ENTITY)) {
                pose.push();
                pose.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(180.0F - body));
                if (ops.root != null) pose.multiplyPositionMatrix(new Matrix4f().set(ops.root));
                draw(ops, YogBodyWire.ANCHOR_ENTITY, null, pose, buffers, light);
                pose.pop();
            }
            return true;
        }
        DRAWING.push(new State(ops));
        return false;
    }

    /** After vanilla turned the body: the mods' turn, then their entity-space meshes. */
    public static void rotated(MatrixStack pose, VertexConsumerProvider buffers, int light) {
        State s = DRAWING.peek();
        if (s == null || s.ops == Ops.NONE) return;
        if (s.ops.root != null) pose.multiplyPositionMatrix(new Matrix4f().set(s.ops.root));
        draw(s.ops, YogBodyWire.ANCHOR_ENTITY, null, pose, buffers, light);
    }

    /** After vanilla animated the model: the mods' part changes. */
    public static void posed(EntityModel<?> model) {
        State s = DRAWING.peek();
        if (s == null || s.ops.bones.isEmpty()) return;
        Map<String, ModelPart[]> parts = parts(model);
        for (BoneOp o : s.ops.bones) {
            ModelPart[] chain = parts.get(o.name);
            if (chain == null) continue;
            ModelPart p = chain[chain.length - 1];
            s.saved.add(new Saved(p));
            if ((o.flags & YogBodyWire.BONE_SET_ROT) != 0) {
                p.pitch = o.rx; p.yaw = o.ry; p.roll = o.rz;
            } else {
                p.pitch += o.rx; p.yaw += o.ry; p.roll += o.rz;
            }
            if ((o.flags & YogBodyWire.BONE_SET_POS) != 0) {
                p.pivotX = o.px; p.pivotY = o.py; p.pivotZ = o.pz;
            } else {
                p.pivotX += o.px; p.pivotY += o.py; p.pivotZ += o.pz;
            }
            if ((o.flags & YogBodyWire.BONE_SCALE) != 0) {
                p.xScale *= o.sx; p.yScale *= o.sy; p.zScale *= o.sz;
            }
            if ((o.flags & YogBodyWire.BONE_HIDE) != 0) p.visible = false;
            if ((o.flags & YogBodyWire.BONE_SHOW) != 0) p.visible = true;
        }
    }

    /** After the model and its layers: the mods' model- and part-space meshes. */
    public static void drawn(EntityModel<?> model, MatrixStack pose, VertexConsumerProvider buffers, int light) {
        State s = DRAWING.peek();
        if (s == null || s.ops.meshes.isEmpty()) return;
        draw(s.ops, YogBodyWire.ANCHOR_MODEL, null, pose, buffers, light);
        if (s.ops.has(YogBodyWire.ANCHOR_BONE)) draw(s.ops, YogBodyWire.ANCHOR_BONE, parts(model), pose, buffers, light);
    }

    /** Render RETURN: the parts back as vanilla left them. */
    public static void end() {
        State s = DRAWING.poll();
        if (s == null) return;
        for (int i = s.saved.size() - 1; i >= 0; i--) s.saved.get(i).restore();
    }

    private static void draw(Ops ops, int anchor, Map<String, ModelPart[]> parts, MatrixStack pose, VertexConsumerProvider buffers, int light) {
        for (Mesh m : ops.meshes) {
            if (m.anchor != anchor) continue;
            ModelPart[] chain = anchor == YogBodyWire.ANCHOR_BONE ? parts.get(m.bone) : null;
            if (anchor == YogBodyWire.ANCHOR_BONE && chain == null) continue;
            pose.push();
            if (chain != null) for (ModelPart p : chain) p.rotate(pose);
            if (anchor != YogBodyWire.ANCHOR_ENTITY) pose.scale(1 / 16.0F, 1 / 16.0F, 1 / 16.0F);
            emit(m, pose, buffers, light);
            pose.pop();
        }
    }

    private static void emit(Mesh m, MatrixStack pose, VertexConsumerProvider buffers, int light) {
        Identifier tex = Identifier.tryParse(m.texture);
        if (tex == null) return;
        RenderLayer type = switch (m.mode) {
            case YogBodyWire.MODE_TRANSLUCENT -> RenderLayer.getEntityTranslucent(tex);
            case YogBodyWire.MODE_EMISSIVE -> RenderLayer.getEyes(tex);
            default -> RenderLayer.getEntityCutoutNoCull(tex);
        };
        VertexConsumer vc = m.glint ? ItemRenderer.getArmorGlintConsumer(buffers, type, false, true) : buffers.getBuffer(type);
        int lit = m.mode == YogBodyWire.MODE_EMISSIVE ? LightmapTextureManager.MAX_LIGHT_COORDINATE : light;
        int a = m.argb >>> 24, r = m.argb >> 16 & 0xFF, g = m.argb >> 8 & 0xFF, b = m.argb & 0xFF;
        Matrix4f mat = pose.peek().getPositionMatrix();
        Matrix3f nm = pose.peek().getNormalMatrix();
        float[] v = m.verts;
        for (int i = 0; i + 8 <= v.length; i += 8) {
            vc.vertex(mat, v[i], v[i + 1], v[i + 2]).color(r, g, b, a).texture(v[i + 3], v[i + 4])
                    .overlay(OverlayTexture.DEFAULT_UV).light(lit).normal(nm, v[i + 5], v[i + 6], v[i + 7]).next();
        }
    }

    // ── Parts by name ─────────────────────────────────────────────────────────

    private static final Map<EntityModel<?>, Map<String, ModelPart[]>> PARTS = new WeakHashMap<>();

    /** A model's parts by name, each with the parts it hangs on (outermost first). */
    private static Map<String, ModelPart[]> parts(EntityModel<?> model) {
        return PARTS.computeIfAbsent(model, m -> {
            Map<String, ModelPart[]> out = new LinkedHashMap<>();
            if (m instanceof SinglePartEntityModel<?> h) walk(h.getPart(), "root", new ModelPart[0], out);
            if (m instanceof BipedEntityModel<?> h) {
                out.put("head", new ModelPart[] {h.head});
                out.put("hat", new ModelPart[] {h.hat});
                out.put("body", new ModelPart[] {h.body});
                out.put("right_arm", new ModelPart[] {h.rightArm});
                out.put("left_arm", new ModelPart[] {h.leftArm});
                out.put("right_leg", new ModelPart[] {h.rightLeg});
                out.put("left_leg", new ModelPart[] {h.leftLeg});
            }
            if (m instanceof PlayerEntityModel<?> p) {
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
        ModelPart[] chain = Arrays.copyOf(above, above.length + 1);
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
        Entity d = DUMMIES.get(e.getUuid());
        if (d == null || d.getWorld() != e.getWorld() || !typeId(d).equals(type)) {
            d = EntityType.get(type).map(t -> (Entity) t.create(e.getWorld())).orElse(null);
            if (d == null) return null;
            d.setSilent(true);
            DUMMIES.put(e.getUuid(), d);
        }
        d.setPosition(e.getX(), e.getY(), e.getZ());
        d.prevX = e.prevX; d.prevY = e.prevY; d.prevZ = e.prevZ;
        d.lastRenderX = e.lastRenderX; d.lastRenderY = e.lastRenderY; d.lastRenderZ = e.lastRenderZ;
        d.setYaw(e.getYaw()); d.prevYaw = e.prevYaw;
        d.setPitch(e.getPitch()); d.prevPitch = e.prevPitch;
        d.age = e.age;
        d.setOnGround(e.isOnGround());
        d.setPose(e.getPose());
        d.setSneaking(e.isSneaking());
        d.setSprinting(e.isSprinting());
        if (d instanceof LivingEntity l) {
            l.bodyYaw = e.bodyYaw; l.prevBodyYaw = e.prevBodyYaw;
            l.headYaw = e.headYaw; l.prevHeadYaw = e.prevHeadYaw;
            LimbAnimatorAccess to = (LimbAnimatorAccess) l.limbAnimator, from = (LimbAnimatorAccess) e.limbAnimator;
            to.yog$setPrevSpeed(from.yog$prevSpeed());
            to.yog$setSpeed(from.yog$speed());
            to.yog$setPos(from.yog$pos());
            l.handSwingProgress = e.handSwingProgress; l.lastHandSwingProgress = e.lastHandSwingProgress;
            l.handSwinging = e.handSwinging; l.handSwingTicks = e.handSwingTicks;
            l.hurtTime = e.hurtTime; l.deathTime = e.deathTime;
            for (EquipmentSlot slot : EquipmentSlot.values()) {
                ItemStack want = e.getEquippedStack(slot);
                if (!ItemStack.areEqual(l.getEquippedStack(slot), want)) l.equipStack(slot, want.copy());
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
                MatrixStack pose = new MatrixStack();
                pose.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(body - 180.0F));
                Buffers buffers = new Buffers();
                @SuppressWarnings("unchecked")
                EntityRenderer<Entity> r = target == e
                        ? (EntityRenderer<Entity>) (Object) self
                        : (EntityRenderer<Entity>) MinecraftClient.getInstance().getEntityRenderDispatcher().getRenderer(target);
                r.render(target, 0, partial, pose, buffers, light);
                Captured c = new Captured(source);
                buffers.sinks.forEach((tex, sink) -> c.meshes.put(tex, sink.quads()));
                out.add(c);
            }
        } finally {
            capturing--;
        }
        YogBodyWire.captured(e.getUuid(), out);
    }

    /** Buffers that keep what's drawn into them, by texture. */
    private static final class Buffers implements VertexConsumerProvider {
        final Map<String, YogBodyWire.Sink> sinks = new LinkedHashMap<>();
        private static final YogBodyWire.Sink NOWHERE = new YogBodyWire.Sink();

        @Override
        public VertexConsumer getBuffer(RenderLayer type) {
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
        public VertexConsumer texture(float u, float v) {
            sink.uv(u, v);
            return this;
        }

        @Override
        public VertexConsumer overlay(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer light(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer normal(float x, float y, float z) {
            sink.normal(x, y, z);
            return this;
        }

        @Override
        public void next() {
            sink.end();
        }

        @Override
        public void fixedColor(int r, int g, int b, int a) {
        }

        @Override
        public void unfixColor() {
        }
    }

    // ── Camera ────────────────────────────────────────────────────────────────

    /** Degrees the view tilts; the field of view's factor (from the last frame's camera). */
    public static float roll;
    public static float fov = 1;

    /**
     * Camera.update TAIL: {right, up, forward, wx, wy, wz, yaw, pitch, roll,
     * fov} from the mods, or null.
     */
    public static float[] camera(Entity entity, float partial, Vec3d pos, float yaw, float pitch, boolean detached) {
        float[] o = entity == null ? null : NativeBridge.nativeCamera(entity.getUuidAsString(), partial, pos.x, pos.y, pos.z,
                yaw, pitch, detached, NativeBridge.entityFlags(entity));
        roll = o == null ? 0 : o[8];
        fov = o == null ? 1 : o[9];
        return o;
    }

    // ── Movement ──────────────────────────────────────────────────────────────

    /** LivingEntity.travel HEAD: true if a mod moved the local player instead. */
    public static boolean travel(LivingEntity self) {
        if (!self.getWorld().isClient || !(self instanceof ClientPlayerEntity p) || p != MinecraftClient.getInstance().player) return false;
        Vec3d v = p.getVelocity();
        Vec3d look = p.getRotationVector();
        float gravity = v.y <= 0 && p.hasStatusEffect(StatusEffects.SLOW_FALLING) ? 0.01F : 0.08F;
        float[] out = NativeBridge.nativeTravel(p.getUuidAsString(), (float) v.x, (float) v.y, (float) v.z, (float) look.x, (float) look.y, (float) look.z,
                p.getYaw(), p.getPitch(), p.forwardSpeed, p.sidewaysSpeed, p.input.jumping, p.isSneaking(), gravity, NativeBridge.entityFlags(p));
        if (out == null) return false;
        Vec3d move = new Vec3d(out[0], out[1], out[2]);
        p.setVelocity(move);
        p.move(MovementType.SELF, move);
        p.updateLimbs(false);
        return true;
    }
}
