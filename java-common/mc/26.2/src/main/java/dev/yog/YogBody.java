package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import dev.yog.mixin.LimbAnimatorAccess;
import dev.yog.mixin.ModelPartAccess;
import dev.yog.util.YogBodyWire;
import dev.yog.util.YogBodyWire.BoneOp;
import dev.yog.util.YogBodyWire.Captured;
import dev.yog.util.YogBodyWire.Mesh;
import dev.yog.util.YogBodyWire.Ops;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.WeakHashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.EntityModel;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.util.Mth;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MoverType;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.Vec3;
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

    /**
     * What the mods said about one entity this frame, kept on its render state
     * (since 1.21.2 an entity is extracted first and drawn from that state later).
     */
    public static final class Frame {
        final Ops ops;
        /** The stand-in drawn in the entity's place (a disguise), and its renderer. */
        final EntityRenderState disguise;
        final EntityRenderer<?, ? super EntityRenderState> disguiseRenderer;
        /** The overlay meshes are drawn with: a mod entity's hurt flash, else none. */
        final int overlay;

        Frame(Ops ops, EntityRenderState disguise, EntityRenderer<?, ? super EntityRenderState> disguiseRenderer, int overlay) {
            this.ops = ops;
            this.disguise = disguise;
            this.disguiseRenderer = disguiseRenderer;
            this.overlay = overlay;
        }
    }

    /** The mods' answer kept on {@code state} this frame, if any. */
    private static Frame frameOf(EntityRenderState state) {
        return (Frame) ((YogBodyState) state).yog$frame();
    }

    private static void setFrame(EntityRenderState state, Frame frame) {
        ((YogBodyState) state).yog$setFrame(frame);
    }

    /** Drawing a capture or a stand-in: the mods aren't asked. */
    private static int capturing;
    /** Model space is a sixteenth of a block per unit. */
    private static final float MODEL_UNIT = 1 / 16.0F;
    /** Where vanilla puts the model's origin below the entity's, in blocks. */
    private static final float MODEL_DROP = 1.501F;
    private static final float HALF_TURN = 180.0F;

    private static String typeId(Entity e) {
        return BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString();
    }

    private static float bodyYaw(LivingEntity e, float partial) {
        return Mth.rotLerp(partial, e.yBodyRotO, e.yBodyRot);
    }

    /** What the mods say about entity {@code e} this frame, if it's tracked. */
    private static Ops ask(Entity e, float partial) {
        LivingEntity living = e instanceof LivingEntity l ? l : null;
        float body = living != null ? bodyYaw(living, partial) : Mth.rotLerp(partial, e.yRotO, e.getYRot());
        float head = living != null ? Mth.rotLerp(partial, living.yHeadRotO, living.yHeadRot) : body;
        boolean walks = living != null && living.isAlive() && !living.isPassenger();
        Vec3 v = e.getDeltaMovement();
        byte[] frame = YogBodyWire.frame(e.getUUID(), typeId(e), partial, e.tickCount + partial,
                walks ? living.walkAnimation.position(partial) : 0, walks ? Math.min(1, living.walkAnimation.speed(partial)) : 0,
                head - body, Mth.lerp(partial, e.xRotO, e.getXRot()), body, e.getXRot(),
                Mth.lerp(partial, e.xo, e.getX()), Mth.lerp(partial, e.yo, e.getY()), Mth.lerp(partial, e.zo, e.getZ()),
                (float) v.x, (float) v.y, (float) v.z, NativeBridge.entityFlags(e), e.level().getGameTime() + (double) partial);
        return Ops.parse(NativeBridge.nativeBodyPose(frame));
    }

    /** LivingEntityRenderer.extractRenderState TAIL: asks the mods, keeps the answer on the state. */
    public static void extract(LivingEntityRenderer<?, ?, ?> self, LivingEntity e, LivingEntityRenderState state, float partial) {
        setFrame(state, null);
        if (capturing > 0 || !YogBodyWire.anyTracked() || !YogBodyWire.tracked(e.getUUID(), typeId(e))) return;
        Ops ops = ask(e, partial);
        if (ops == Ops.NONE) return;
        if (!ops.captures.isEmpty()) capture(self, e, ops.captures, partial);
        EntityRenderState disguise = null;
        EntityRenderer<?, ? super EntityRenderState> renderer = null;
        if (ops.disguise != null) {
            Entity d = dummy(e, ops.disguise);
            if (d != null) {
                capturing++; // the stand-in isn't asked about
                try {
                    renderer = rendererOf(d);
                    disguise = stateOf(d, partial);
                } finally {
                    capturing--;
                }
            }
        }
        setFrame(state, new Frame(ops, disguise, renderer, OverlayTexture.NO_OVERLAY));
    }

    @SuppressWarnings("unchecked")
    private static EntityRenderer<?, ? super EntityRenderState> rendererOf(Entity e) {
        return (EntityRenderer<?, ? super EntityRenderState>) Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(e);
    }

    @SuppressWarnings("unchecked")
    private static EntityRenderState stateOf(Entity e, float partial) {
        return ((EntityRenderer<Entity, EntityRenderState>) Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(e))
                .createRenderState(e, partial);
    }

    /** Submit HEAD: the disguise; true if the entity itself isn't drawn. */
    public static boolean begin(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera) {
        Frame f = frameOf(state);
        if (f == null) return false;
        if (f.disguise != null) f.disguiseRenderer.submit(f.disguise, pose, out, camera);
        if (!f.ops.hide) return false;
        if (f.ops.has(YogBodyWire.ANCHOR_ENTITY)) {
            pose.pushPose();
            pose.scale(state.scale, state.scale, state.scale);
            pose.mulPose(Axis.YP.rotationDegrees(HALF_TURN - state.bodyRot));
            if (f.ops.root != null) pose.mulPose(new Matrix4f().set(f.ops.root));
            draw(f, YogBodyWire.ANCHOR_ENTITY, null, pose, out, state.lightCoords);
            pose.popPose();
        }
        return true;
    }

    /** After vanilla turned the body: the mods' turn, then their entity-space meshes. */
    public static void rotated(LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out) {
        Frame f = frameOf(state);
        if (f == null) return;
        if (f.ops.root != null) pose.mulPose(new Matrix4f().set(f.ops.root));
        draw(f, YogBodyWire.ANCHOR_ENTITY, null, pose, out, state.lightCoords);
    }

    /** After a model was animated for {@code state}: the mods' part changes (it's reset at each animation). */
    public static void posed(Model<?> model, Object state) {
        if (!(state instanceof EntityRenderState s)) return;
        Frame f = frameOf(s);
        if (f == null || f.ops.bones.isEmpty()) return;
        Map<String, ModelPart[]> parts = parts(model);
        for (BoneOp o : f.ops.bones) {
            ModelPart[] chain = parts.get(o.name);
            if (chain == null) continue;
            ModelPart p = chain[chain.length - 1];
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

    /** Before the model space is left: the mods' model- and part-space meshes. */
    @SuppressWarnings("unchecked")
    public static void drawn(EntityModel<?> model, LivingEntityRenderState state, PoseStack pose, SubmitNodeCollector out) {
        Frame f = frameOf(state);
        if (f == null || f.ops.meshes.isEmpty()) return;
        draw(f, YogBodyWire.ANCHOR_MODEL, null, pose, out, state.lightCoords);
        if (!f.ops.has(YogBodyWire.ANCHOR_BONE)) return;
        // The parts as they'll be drawn: animated, then changed by the mods.
        ((EntityModel<LivingEntityRenderState>) model).setupAnim(state);
        posed(model, state);
        draw(f, YogBodyWire.ANCHOR_BONE, parts(model), pose, out, state.lightCoords);
    }

    private static void draw(Frame f, int anchor, Map<String, ModelPart[]> parts, PoseStack pose, SubmitNodeCollector out, int light) {
        for (Mesh m : f.ops.meshes) {
            if (m.anchor != anchor) continue;
            ModelPart[] chain = anchor == YogBodyWire.ANCHOR_BONE ? parts.get(m.bone) : null;
            if (anchor == YogBodyWire.ANCHOR_BONE && chain == null) continue;
            pose.pushPose();
            if (chain != null) for (ModelPart p : chain) p.translateAndRotate(pose);
            if (anchor != YogBodyWire.ANCHOR_ENTITY) pose.scale(MODEL_UNIT, MODEL_UNIT, MODEL_UNIT);
            emit(m, pose, out, light, f.overlay);
            pose.popPose();
        }
    }

    private static void emit(Mesh m, PoseStack pose, SubmitNodeCollector out, int light, int overlay) {
        Identifier tex = Identifier.tryParse(m.texture);
        if (tex == null) return;
        RenderType type = switch (m.mode) {
            case YogBodyWire.MODE_TRANSLUCENT -> RenderTypes.entityTranslucent(tex);
            case YogBodyWire.MODE_EMISSIVE -> RenderTypes.eyes(tex);
            default -> RenderTypes.entityCutout(tex);
        };
        int lit = m.mode == YogBodyWire.MODE_EMISSIVE ? LightCoordsUtil.FULL_BRIGHT : light;
        int a = m.argb >>> 24, r = m.argb >> 16 & 0xFF, g = m.argb >> 8 & 0xFF, b = m.argb & 0xFF;
        float[] v = m.verts;
        SubmitNodeCollector.CustomGeometryRenderer quads = (last, vc) -> {
            for (int i = 0; i + 8 <= v.length; i += 8) {
                vc.addVertex(last, v[i], v[i + 1], v[i + 2]).setColor(r, g, b, a).setUv(v[i + 3], v[i + 4])
                        .setOverlay(overlay).setLight(lit).setNormal(last, v[i + 5], v[i + 6], v[i + 7]);
            }
        };
        out.submitCustomGeometry(pose, type, quads);
        if (m.glint) out.submitCustomGeometry(pose, RenderTypes.armorEntityGlint(), quads);
    }

    /**
     * A mod's own entity ({@link YogEntityRenderer}), extracted: what the mods
     * draw for it, kept on its state; nothing when they draw nothing.
     */
    public static void extractEntity(Entity e, EntityRenderState state, float partial) {
        setFrame(state, null);
        if (capturing > 0 || !YogBodyWire.tracked(e.getUUID(), typeId(e))) return;
        Ops ops = ask(e, partial);
        if (ops == Ops.NONE) return;
        int overlay = e instanceof LivingEntity l
                ? OverlayTexture.pack(0, l.hurtTime > 0 || l.deathTime > 0)
                : OverlayTexture.NO_OVERLAY;
        ((YogBodyState) state).yog$setBodyYaw(e instanceof LivingEntity l ? bodyYaw(l, partial) : Mth.rotLerp(partial, e.yRotO, e.getYRot()));
        setFrame(state, new Frame(ops, null, null, overlay));
    }


    /** A mod's own entity: no vanilla model, only what the mods draw — entity space after its yaw, then model space. */
    public static void drawEntity(EntityRenderState state, PoseStack pose, SubmitNodeCollector out) {
        Frame f = frameOf(state);
        if (f == null) return;
        pose.pushPose();
        pose.mulPose(Axis.YP.rotationDegrees(HALF_TURN - ((YogBodyState) state).yog$bodyYaw()));
        if (f.ops.root != null) pose.mulPose(new Matrix4f().set(f.ops.root));
        draw(f, YogBodyWire.ANCHOR_ENTITY, null, pose, out, state.lightCoords);
        pose.scale(-1.0F, -1.0F, 1.0F);
        pose.translate(0.0F, -MODEL_DROP, 0.0F);
        draw(f, YogBodyWire.ANCHOR_MODEL, null, pose, out, state.lightCoords);
        pose.popPose();
    }

    // ── Parts by name ─────────────────────────────────────────────────────────

    private static final Map<Model<?>, Map<String, ModelPart[]>> PARTS = new WeakHashMap<>();

    /** A model's parts by name (as its layer definition names them), each with the parts it hangs on (outermost first). */
    private static Map<String, ModelPart[]> parts(Model<?> model) {
        return PARTS.computeIfAbsent(model, m -> {
            Map<String, ModelPart[]> out = new LinkedHashMap<>();
            walk(m.root(), "root", new ModelPart[0], out);
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
        Entity d = DUMMIES.get(e.getUUID());
        if (d == null || d.level() != e.level() || !typeId(d).equals(type)) {
            Identifier id = Identifier.tryParse(type);
            d = id == null ? null : BuiltInRegistries.ENTITY_TYPE.getOptional(id)
                    .map(t -> (Entity) t.create(e.level(), EntitySpawnReason.LOAD)).orElse(null);
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
            LimbAnimatorAccess to = (LimbAnimatorAccess) l.walkAnimation, from = (LimbAnimatorAccess) e.walkAnimation;
            to.yog$setPrevSpeed(from.yog$prevSpeed());
            to.yog$setSpeed(from.yog$speed());
            to.yog$setPos(from.yog$pos());
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

    private static void capture(LivingEntityRenderer<?, ?, ?> self, LivingEntity e, List<String> sources, float partial) {
        List<Captured> out = new ArrayList<>();
        float body = bodyYaw(e, partial);
        capturing++;
        try {
            for (String source : sources) {
                Entity target = source.equals(SELF) ? e : dummy(e, source);
                if (target == null) continue;
                PoseStack pose = new PoseStack();
                pose.mulPose(Axis.YP.rotationDegrees(body - HALF_TURN));
                YogBodyCapture keep = new YogBodyCapture();
                @SuppressWarnings("unchecked")
                EntityRenderer<Entity, EntityRenderState> r = target == e
                        ? (EntityRenderer<Entity, EntityRenderState>) (Object) self
                        : (EntityRenderer<Entity, EntityRenderState>) (Object) rendererOf(target);
                r.submit(r.createRenderState(target, partial), pose, keep, new CameraRenderState());
                Captured c = new Captured(source);
                keep.sinks.forEach((tex, sink) -> c.meshes.put(tex, sink.quads()));
                out.add(c);
            }
        } finally {
            capturing--;
        }
        YogBodyWire.captured(e.getUUID(), out);
    }

    /** A capture's source naming the entity itself. */
    private static final String SELF = "self";

    // ── Camera ────────────────────────────────────────────────────────────────

    /** Degrees the view tilts; the field of view's factor (from the last frame's camera). */
    public static float roll;
    public static float fov = 1;

    /**
     * Camera.update TAIL: {right, up, forward, wx, wy, wz, yaw, pitch, roll,
     * fov} from the mods, or null.
     */
    public static float[] camera(Entity entity, float partial, Vec3 pos, float yaw, float pitch, boolean detached) {
        float[] o = entity == null ? null : NativeBridge.nativeCamera(entity.getStringUUID(), partial, pos.x, pos.y, pos.z,
                yaw, pitch, detached, NativeBridge.entityFlags(entity));
        roll = o == null ? 0 : o[8];
        fov = o == null ? 1 : o[9];
        return o;
    }

    // ── Movement ──────────────────────────────────────────────────────────────

    /** LivingEntity.travel HEAD: true if a mod moved the local player instead. */
    public static boolean travel(LivingEntity self) {
        if (!self.level().isClientSide() || !(self instanceof LocalPlayer p) || p != Minecraft.getInstance().player) return false;
        Vec3 v = p.getDeltaMovement();
        Vec3 look = p.getLookAngle();
        float gravity = v.y <= 0 && p.hasEffect(MobEffects.SLOW_FALLING) ? 0.01F : 0.08F;
        float[] out = NativeBridge.nativeTravel(p.getStringUUID(), (float) v.x, (float) v.y, (float) v.z, (float) look.x, (float) look.y, (float) look.z,
                p.getYRot(), p.getXRot(), p.zza, p.xxa, p.input.keyPresses.jump(), p.isShiftKeyDown(), gravity, NativeBridge.entityFlags(p));
        if (out == null) return false;
        Vec3 move = new Vec3(out[0], out[1], out[2]);
        p.setDeltaMovement(move);
        p.move(MoverType.SELF, move);
        p.calculateEntityAnimation(false);
        return true;
    }
}
