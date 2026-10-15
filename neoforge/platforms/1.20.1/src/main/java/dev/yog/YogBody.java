package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Axis;
import dev.yog.mixin.ModelPartAccess;
import dev.yog.mixin.WalkAnimationAccess;
import dev.yog.util.YogBodyWire;
import dev.yog.util.YogBodyWire.BoneOp;
import dev.yog.util.YogBodyWire.Captured;
import dev.yog.util.YogBodyWire.Mesh;
import dev.yog.util.YogBodyWire.Ops;
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
import net.minecraft.core.registries.BuiltInRegistries;
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
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.ViewportEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;
import org.joml.Matrix3f;
import org.joml.Matrix4f;

/**
 * yog-body on this host (see {@code docs/design/yog-body.md}): asks the mods
 * about tracked entities' bodies while they're drawn, about the camera and
 * about the local player's travel step, and does what they say.
 */
@Mod.EventBusSubscriber(modid = "yog", value = Dist.CLIENT)
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
        return BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString();
    }

    private static float bodyYaw(LivingEntity e, float partial) {
        return Mth.rotLerp(partial, e.yBodyRotO, e.yBodyRot);
    }

    /** Render HEAD: asks the mods; true if the entity itself isn't drawn. */
    public static boolean begin(LivingEntityRenderer<?, ?> self, LivingEntity e, float yaw, float partial,
                                PoseStack pose, MultiBufferSource buffers, int light) {
        if (capturing > 0 || !YogBodyWire.anyTracked() || !YogBodyWire.tracked(e.getUUID(), typeId(e))) {
            DRAWING.push(State.NONE);
            return false;
        }
        float body = bodyYaw(e, partial);
        float head = Mth.rotLerp(partial, e.yHeadRotO, e.yHeadRot);
        boolean alive = e.isAlive() && !e.isPassenger();
        Vec3 v = e.getDeltaMovement();
        byte[] frame = YogBodyWire.frame(e.getUUID(), typeId(e), partial, e.tickCount + partial,
                alive ? e.walkAnimation.position(partial) : 0, alive ? Math.min(1, e.walkAnimation.speed(partial)) : 0,
                head - body, Mth.lerp(partial, e.xRotO, e.getXRot()), body, e.getXRot(),
                Mth.lerp(partial, e.xo, e.getX()), Mth.lerp(partial, e.yo, e.getY()), Mth.lerp(partial, e.zo, e.getZ()),
                (float) v.x, (float) v.y, (float) v.z, NativeBridge.entityFlags(e));
        Ops ops = Ops.parse(NativeBridge.nativeBodyPose(frame));
        if (!ops.captures.isEmpty()) capture(self, e, ops.captures, partial, light);
        if (ops.disguise != null) {
            Entity d = dummy(e, ops.disguise);
            if (d != null) {
                capturing++; // the stand-in isn't asked about
                try {
                    Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(d).render(d, yaw, partial, pose, buffers, light);
                } finally {
                    capturing--;
                }
            }
        }
        if (ops.hide) {
            if (ops.has(YogBodyWire.ANCHOR_ENTITY)) {
                pose.pushPose();
                pose.mulPose(Axis.YP.rotationDegrees(180.0F - body));
                if (ops.root != null) pose.mulPoseMatrix(new Matrix4f().set(ops.root));
                draw(ops, YogBodyWire.ANCHOR_ENTITY, null, pose, buffers, light);
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
        if (s == null || s.ops == Ops.NONE) return;
        if (s.ops.root != null) pose.mulPoseMatrix(new Matrix4f().set(s.ops.root));
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
                p.xRot = o.rx; p.yRot = o.ry; p.zRot = o.rz;
            } else {
                p.xRot += o.rx; p.yRot += o.ry; p.zRot += o.rz;
            }
            if ((o.flags & YogBodyWire.BONE_SET_POS) != 0) {
                p.x = o.px; p.y = o.py; p.z = o.pz;
            } else {
                p.x += o.px; p.y += o.py; p.z += o.pz;
            }
            if ((o.flags & YogBodyWire.BONE_SCALE) != 0) {
                p.xScale *= o.sx; p.yScale *= o.sy; p.zScale *= o.sz;
            }
            if ((o.flags & YogBodyWire.BONE_HIDE) != 0) p.visible = false;
            if ((o.flags & YogBodyWire.BONE_SHOW) != 0) p.visible = true;
        }
    }

    /** After the model and its layers: the mods' model- and part-space meshes. */
    public static void drawn(EntityModel<?> model, PoseStack pose, MultiBufferSource buffers, int light) {
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

    private static void draw(Ops ops, int anchor, Map<String, ModelPart[]> parts, PoseStack pose, MultiBufferSource buffers, int light) {
        for (Mesh m : ops.meshes) {
            if (m.anchor != anchor) continue;
            pose.pushPose();
            if (anchor == YogBodyWire.ANCHOR_BONE) {
                ModelPart[] chain = parts.get(m.bone);
                if (chain == null) {
                    pose.popPose();
                    continue;
                }
                for (ModelPart p : chain) p.translateAndRotate(pose);
            }
            if (anchor != YogBodyWire.ANCHOR_ENTITY) pose.scale(1 / 16.0F, 1 / 16.0F, 1 / 16.0F);
            emit(m, pose, buffers, light);
            pose.popPose();
        }
    }

    private static void emit(Mesh m, PoseStack pose, MultiBufferSource buffers, int light) {
        ResourceLocation tex = ResourceLocation.tryParse(m.texture);
        if (tex == null) return;
        RenderType type = switch (m.mode) {
            case YogBodyWire.MODE_TRANSLUCENT -> RenderType.entityTranslucent(tex);
            case YogBodyWire.MODE_EMISSIVE -> RenderType.eyes(tex);
            default -> RenderType.entityCutoutNoCull(tex);
        };
        VertexConsumer vc = m.glint ? ItemRenderer.getArmorFoilBuffer(buffers, type, false, true) : buffers.getBuffer(type);
        int lit = m.mode == YogBodyWire.MODE_EMISSIVE ? LightTexture.FULL_BRIGHT : light;
        int a = m.argb >>> 24, r = m.argb >> 16 & 0xFF, g = m.argb >> 8 & 0xFF, b = m.argb & 0xFF;
        Matrix4f mat = pose.last().pose();
        Matrix3f nm = pose.last().normal();
        float[] v = m.verts;
        for (int i = 0; i + 8 <= v.length; i += 8) {
            vc.vertex(mat, v[i], v[i + 1], v[i + 2]).color(r, g, b, a).uv(v[i + 3], v[i + 4])
                    .overlayCoords(OverlayTexture.NO_OVERLAY).uv2(lit).normal(nm, v[i + 5], v[i + 6], v[i + 7]).endVertex();
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
        if (d == null || d.level() != e.level() || !typeId(d).equals(type)) {
            d = EntityType.byString(type).map(t -> t.create(e.level())).orElse(null);
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
        d.setOnGround(e.onGround());
        d.setPose(e.getPose());
        d.setShiftKeyDown(e.isShiftKeyDown());
        d.setSprinting(e.isSprinting());
        if (d instanceof LivingEntity l) {
            l.yBodyRot = e.yBodyRot; l.yBodyRotO = e.yBodyRotO;
            l.yHeadRot = e.yHeadRot; l.yHeadRotO = e.yHeadRotO;
            WalkAnimationAccess to = (WalkAnimationAccess) l.walkAnimation, from = (WalkAnimationAccess) e.walkAnimation;
            to.yog$setSpeedOld(from.yog$speedOld());
            to.yog$setSpeed(from.yog$speed());
            to.yog$setPosition(from.yog$position());
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
                pose.mulPose(Axis.YP.rotationDegrees(body - 180.0F));
                Buffers buffers = new Buffers();
                @SuppressWarnings("unchecked")
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

    private static float roll;
    private static float fov = 1;

    /**
     * Camera.setup TAIL: {right, up, forward, wx, wy, wz, yaw, pitch, roll,
     * fov} from the mods, or null.
     */
    public static float[] camera(Entity entity, float partial, Vec3 pos, float yaw, float pitch, boolean detached) {
        float[] o = entity == null ? null : NativeBridge.nativeCamera(entity.getStringUUID(), partial, pos.x, pos.y, pos.z,
                yaw, pitch, detached, NativeBridge.entityFlags(entity));
        roll = o == null ? 0 : o[8];
        fov = o == null ? 1 : o[9];
        return o;
    }

    @SubscribeEvent
    public static void onAngles(ViewportEvent.ComputeCameraAngles event) {
        if (roll != 0) event.setRoll(event.getRoll() + roll);
    }

    @SubscribeEvent
    public static void onFov(ViewportEvent.ComputeFov event) {
        if (fov != 1) event.setFOV(event.getFOV() * fov);
    }

    // ── Movement ──────────────────────────────────────────────────────────────

    /** LivingEntity.travel HEAD: true if a mod moved the local player instead. */
    public static boolean travel(LivingEntity self) {
        if (!self.level().isClientSide || !(self instanceof LocalPlayer p) || p != Minecraft.getInstance().player) return false;
        Vec3 v = p.getDeltaMovement();
        Vec3 look = p.getLookAngle();
        float gravity = v.y <= 0 && p.hasEffect(MobEffects.SLOW_FALLING) ? 0.01F : 0.08F;
        float[] out = NativeBridge.nativeTravel(p.getStringUUID(), (float) v.x, (float) v.y, (float) v.z, (float) look.x, (float) look.y, (float) look.z,
                p.getYRot(), p.getXRot(), p.zza, p.xxa, p.input.jumping, p.isShiftKeyDown(), gravity, NativeBridge.entityFlags(p));
        if (out == null) return false;
        Vec3 move = new Vec3(out[0], out[1], out[2]);
        p.setDeltaMovement(move);
        p.move(MoverType.SELF, move);
        p.calculateEntityAnimation(false);
        return true;
    }
}
