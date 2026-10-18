package dev.yog;

import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.FluidTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.ClickType;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * The local player and what it sees, for Rust mods (ABI 3.9, `client_op`):
 * one call per operation of `YogNames.ClientOp`, arguments and answer as JSON.
 * Movement is the player's own keys held down, so the game sends the
 * server what a player's input would. Called on the client thread (a
 * mod's client tick); every operation answers null without a player.
 */
public final class YogClientOps {
    private YogClientOps() {}

    private static final Gson GSON = new Gson();
    /** At most this many blocks are scanned or sent in one call. */
    private static final long MAX_BLOCKS = 1L << 21;
    /** Blocks that hurt to touch, besides fire and lava. */
    private static final Set<Block> HARMFUL = Set.of(Blocks.CACTUS, Blocks.SWEET_BERRY_BUSH, Blocks.MAGMA_BLOCK, Blocks.WITHER_ROSE, Blocks.POWDER_SNOW);
    /** A face's hit point: this far from the block's centre toward the face. */
    private static final double FACE_OFFSET = 0.5;
    /** The partial tick the attack's charge is read at, as the game's crosshair does. */
    private static final float HALF_TICK = 0.5f;
    /** What a tag reference starts with, in a scan's ids. */
    private static final String TAG_MARK = "#";

    public static String call(String op, String args) {
        try {
            Minecraft mc = Minecraft.getInstance();
            LocalPlayer p = mc.player;
            ClientLevel w = mc.level;
            if (p == null || w == null) return null;
            JsonObject a = args == null || args.isBlank() ? new JsonObject() : parse(args);
            Object answer = switch (op) {
                case YogNames.ClientOp.STATE -> state(p, w);
                case YogNames.ClientOp.DRIVE -> drive(mc.options, a);
                case YogNames.ClientOp.RELEASE -> release(mc.options);
                case YogNames.ClientOp.LOOK -> look(p, a);
                case YogNames.ClientOp.ATTACK -> attack(mc, p, w, a);
                case YogNames.ClientOp.USE -> mc.gameMode.useItem(p, a.has("off_hand") && a.get("off_hand").getAsBoolean() ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND).consumesAction();
                case YogNames.ClientOp.MINE -> mine(mc, p, w, a);
                case YogNames.ClientOp.PLACE -> place(mc, p, a);
                case YogNames.ClientOp.SLOT -> slot(p, a);
                case YogNames.ClientOp.SCAN -> scan(w, a);
                case YogNames.ClientOp.AREA -> area(w, a);
                case YogNames.ClientOp.ENTITIES -> entities(p, w, a);
                case YogNames.ClientOp.INVENTORY -> inventory(p);
                case YogNames.ClientOp.CLICK -> click(mc, p, a);
                case YogNames.ClientOp.CHAT -> chat(mc, a);
                default -> null;
            };
            return answer == null ? null : GSON.toJson(answer);
        } catch (Throwable t) {
            YogLog.warn("client op " + op + ": " + t);
            return null;
        }
    }

    private static JsonObject parse(String args) {
        JsonElement e = JsonParser.parseString(args);
        return e.isJsonObject() ? e.getAsJsonObject() : new JsonObject();
    }

    private static Map<String, Object> state(LocalPlayer p, ClientLevel w) {
        Map<String, Object> m = new HashMap<>();
        Vec3 v = p.getDeltaMovement();
        m.put("x", p.getX());
        m.put("y", p.getY());
        m.put("z", p.getZ());
        m.put("yaw", p.getYRot());
        m.put("pitch", p.getXRot());
        m.put("vx", v.x);
        m.put("vy", v.y);
        m.put("vz", v.z);
        m.put("on_ground", p.onGround());
        m.put("in_water", p.isInWater());
        m.put("eye_height", (double) p.getEyeHeight());
        m.put("health", p.getHealth());
        m.put("food", p.getFoodData().getFoodLevel());
        m.put("dimension", w.dimension().location().toString());
        m.put("slot", p.getInventory().selected);
        m.put("attack_strength", p.getAttackStrengthScale(HALF_TICK));
        return m;
    }

    private static boolean flag(JsonObject a, String key) {
        return a.has(key) && a.get(key).getAsBoolean();
    }

    private static float number(JsonObject a, String key) {
        return a.has(key) && !a.get(key).isJsonNull() ? a.get(key).getAsFloat() : 0f;
    }

    private static Object drive(Options o, JsonObject a) {
        float forward = number(a, "forward");
        float strafe = number(a, "strafe");
        o.keyUp.setDown(forward > 0);
        o.keyDown.setDown(forward < 0);
        o.keyLeft.setDown(strafe > 0);
        o.keyRight.setDown(strafe < 0);
        o.keyJump.setDown(flag(a, "jump"));
        o.keyShift.setDown(flag(a, "sneak"));
        o.keySprint.setDown(flag(a, "sprint"));
        return true;
    }

    private static Object release(Options o) {
        for (KeyMapping k : new KeyMapping[] {o.keyUp, o.keyDown, o.keyLeft, o.keyRight, o.keyJump, o.keyShift, o.keySprint}) {
            k.setDown(false);
        }
        // The keys the player really holds, back.
        KeyMapping.setAll();
        return true;
    }

    private static Object look(LocalPlayer p, JsonObject a) {
        p.setYRot(number(a, "yaw"));
        p.setXRot(Math.max(-90f, Math.min(90f, number(a, "pitch"))));
        return true;
    }

    private static Entity entity(ClientLevel w, String uuid) {
        for (Entity e : w.entitiesForRendering()) {
            if (e.getStringUUID().equals(uuid)) return e;
        }
        return null;
    }

    private static Object attack(Minecraft mc, LocalPlayer p, ClientLevel w, JsonObject a) {
        Entity target = a.has("entity") && !a.get("entity").isJsonNull() ? entity(w, a.get("entity").getAsString())
            : mc.hitResult instanceof EntityHitResult hit ? hit.getEntity() : null;
        if (target != null) mc.gameMode.attack(p, target);
        p.swing(InteractionHand.MAIN_HAND);
        return target != null;
    }

    private static BlockPos pos(JsonObject a) {
        return new BlockPos(a.get("x").getAsInt(), a.get("y").getAsInt(), a.get("z").getAsInt());
    }

    private static Direction face(JsonObject a) {
        Direction d = a.has("face") ? Direction.byName(a.get("face").getAsString()) : null;
        return d == null ? Direction.UP : d;
    }

    private static Object mine(Minecraft mc, LocalPlayer p, ClientLevel w, JsonObject a) {
        if (flag(a, "stop")) {
            mc.gameMode.stopDestroyBlock();
            return false;
        }
        BlockPos at = pos(a);
        mc.gameMode.continueDestroyBlock(at, face(a));
        p.swing(InteractionHand.MAIN_HAND);
        return !w.getBlockState(at).isAir();
    }

    private static Object place(Minecraft mc, LocalPlayer p, JsonObject a) {
        BlockPos at = pos(a);
        Direction d = face(a);
        Vec3 hitAt = Vec3.atCenterOf(at).add(Vec3.atLowerCornerOf(d.getNormal()).scale(FACE_OFFSET));
        boolean placed = mc.gameMode.useItemOn(p, InteractionHand.MAIN_HAND, new BlockHitResult(hitAt, d, at, false)).consumesAction();
        if (placed) p.swing(InteractionHand.MAIN_HAND);
        return placed;
    }

    private static Object slot(LocalPlayer p, JsonObject a) {
        int s = a.get("slot").getAsInt();
        if (s < 0 || s > 8) return false;
        p.getInventory().selected = s;
        return true;
    }

    /** `min`, `max` of a request, ordered; null when the box is too big. */
    private static BlockPos[] box(JsonObject a) {
        JsonArray lo = a.getAsJsonArray("min");
        JsonArray hi = a.getAsJsonArray("max");
        BlockPos min = new BlockPos(Math.min(lo.get(0).getAsInt(), hi.get(0).getAsInt()), Math.min(lo.get(1).getAsInt(), hi.get(1).getAsInt()), Math.min(lo.get(2).getAsInt(), hi.get(2).getAsInt()));
        BlockPos max = new BlockPos(Math.max(lo.get(0).getAsInt(), hi.get(0).getAsInt()), Math.max(lo.get(1).getAsInt(), hi.get(1).getAsInt()), Math.max(lo.get(2).getAsInt(), hi.get(2).getAsInt()));
        long volume = (long) (max.getX() - min.getX() + 1) * (max.getY() - min.getY() + 1) * (max.getZ() - min.getZ() + 1);
        return volume > MAX_BLOCKS ? null : new BlockPos[] {min, max};
    }

    private static boolean loaded(ClientLevel w, BlockPos at) {
        return w.getChunkSource().hasChunk(at.getX() >> 4, at.getZ() >> 4);
    }

    private static Object scan(ClientLevel w, JsonObject a) {
        BlockPos[] b = box(a);
        if (b == null) return null;
        Set<ResourceLocation> ids = new HashSet<>();
        Set<TagKey<Block>> tags = new HashSet<>();
        for (JsonElement e : a.getAsJsonArray("ids")) {
            String s = e.getAsString();
            ResourceLocation id = ResourceLocation.tryParse(s.startsWith(TAG_MARK) ? s.substring(TAG_MARK.length()) : s);
            if (id == null) continue;
            if (s.startsWith(TAG_MARK)) tags.add(TagKey.create(Registries.BLOCK, id));
            else ids.add(id);
        }
        JsonArray out = new JsonArray();
        for (BlockPos at : BlockPos.betweenClosed(b[0], b[1])) {
            if (!loaded(w, at)) continue;
            BlockState st = w.getBlockState(at);
            ResourceLocation id = BuiltInRegistries.BLOCK.getKey(st.getBlock());
            boolean hit = ids.contains(id) || tags.stream().anyMatch(st::is);
            if (!hit) continue;
            JsonObject f = new JsonObject();
            f.addProperty("x", at.getX());
            f.addProperty("y", at.getY());
            f.addProperty("z", at.getZ());
            f.addProperty("id", id.toString());
            out.add(f);
        }
        return out;
    }

    private static Object area(ClientLevel w, JsonObject a) {
        BlockPos[] b = box(a);
        if (b == null) return null;
        Map<BlockState, Integer> index = new HashMap<>();
        JsonArray palette = new JsonArray();
        JsonArray blocks = new JsonArray();
        for (int y = b[0].getY(); y <= b[1].getY(); y++)
            for (int z = b[0].getZ(); z <= b[1].getZ(); z++)
                for (int x = b[0].getX(); x <= b[1].getX(); x++) {
                    BlockPos at = new BlockPos(x, y, z);
                    BlockState st = loaded(w, at) ? w.getBlockState(at) : Blocks.BEDROCK.defaultBlockState();
                    Integer i = index.get(st);
                    if (i == null) {
                        i = index.size();
                        index.put(st, i);
                        palette.add(kind(w, at, st));
                    }
                    blocks.add(i);
                }
        JsonObject out = new JsonObject();
        out.add("min", GSON.toJsonTree(new int[] {b[0].getX(), b[0].getY(), b[0].getZ()}));
        out.add("max", GSON.toJsonTree(new int[] {b[1].getX(), b[1].getY(), b[1].getZ()}));
        out.add("palette", palette);
        out.add("blocks", blocks);
        return out;
    }

    private static JsonObject kind(ClientLevel w, BlockPos at, BlockState st) {
        JsonObject k = new JsonObject();
        k.addProperty("id", BuiltInRegistries.BLOCK.getKey(st.getBlock()).toString());
        k.addProperty("solid", !st.getCollisionShape(w, at).isEmpty());
        k.addProperty("liquid", !st.getFluidState().isEmpty());
        k.addProperty("climbable", st.is(BlockTags.CLIMBABLE));
        k.addProperty("harmful", st.is(BlockTags.FIRE) || st.getFluidState().is(FluidTags.LAVA) || HARMFUL.contains(st.getBlock()));
        return k;
    }

    private static Object entities(LocalPlayer p, ClientLevel w, JsonObject a) {
        double r = a.has("radius") ? a.get("radius").getAsDouble() : 0;
        JsonArray out = new JsonArray();
        for (Entity e : w.entitiesForRendering()) {
            if (e == p || e.distanceToSqr(p) > r * r) continue;
            JsonObject o = new JsonObject();
            o.addProperty("uuid", e.getStringUUID());
            o.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
            o.addProperty("x", e.getX());
            o.addProperty("y", e.getY());
            o.addProperty("z", e.getZ());
            o.addProperty("eye_height", (double) e.getEyeHeight());
            if (e instanceof LivingEntity l) o.addProperty("health", l.getHealth());
            if (e instanceof Player pl) o.addProperty("name", pl.getGameProfile().getName());
            o.addProperty("hostile", e instanceof Enemy);
            out.add(o);
        }
        return out;
    }

    private static Object inventory(LocalPlayer p) {
        JsonArray out = new JsonArray();
        var inv = p.getInventory();
        for (int i = 0; i < inv.getContainerSize(); i++) {
            ItemStack st = inv.getItem(i);
            if (st.isEmpty()) continue;
            JsonObject o = new JsonObject();
            o.addProperty("slot", i);
            o.addProperty("item", BuiltInRegistries.ITEM.getKey(st.getItem()).toString());
            o.addProperty("count", st.getCount());
            out.add(o);
        }
        return out;
    }

    private static Object click(Minecraft mc, LocalPlayer p, JsonObject a) {
        ClickType mode = ClickType.valueOf(a.get("mode").getAsString().toUpperCase(java.util.Locale.ROOT));
        mc.gameMode.handleInventoryMouseClick(p.containerMenu.containerId, a.get("slot").getAsInt(), a.get("button").getAsInt(), mode, p);
        return true;
    }

    private static Object chat(Minecraft mc, JsonObject a) {
        String text = a.get("text").getAsString();
        if (text.startsWith("/")) mc.getConnection().sendCommand(text.substring(1));
        else mc.getConnection().sendChat(text);
        return true;
    }
}
