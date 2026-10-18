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

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.Blocks;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.Entity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.mob.Monster;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.registry.tag.BlockTags;
import net.minecraft.registry.tag.FluidTags;
import net.minecraft.registry.tag.TagKey;
import net.minecraft.screen.slot.SlotActionType;
import net.minecraft.util.Hand;
import net.minecraft.util.Identifier;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.EntityHitResult;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.util.math.Vec3d;

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
            MinecraftClient mc = MinecraftClient.getInstance();
            ClientPlayerEntity p = mc.player;
            ClientWorld w = mc.world;
            if (p == null || w == null) return null;
            JsonObject a = args == null || args.isBlank() ? new JsonObject() : parse(args);
            Object answer = switch (op) {
                case YogNames.ClientOp.STATE -> state(p, w);
                case YogNames.ClientOp.DRIVE -> drive(mc.options, a);
                case YogNames.ClientOp.RELEASE -> release(mc.options);
                case YogNames.ClientOp.LOOK -> look(p, a);
                case YogNames.ClientOp.ATTACK -> attack(mc, p, w, a);
                case YogNames.ClientOp.USE -> mc.interactionManager.interactItem(p, a.has("off_hand") && a.get("off_hand").getAsBoolean() ? Hand.OFF_HAND : Hand.MAIN_HAND).isAccepted();
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

    private static Map<String, Object> state(ClientPlayerEntity p, ClientWorld w) {
        Map<String, Object> m = new HashMap<>();
        Vec3d v = p.getVelocity();
        m.put("x", p.getX());
        m.put("y", p.getY());
        m.put("z", p.getZ());
        m.put("yaw", p.getYaw());
        m.put("pitch", p.getPitch());
        m.put("vx", v.x);
        m.put("vy", v.y);
        m.put("vz", v.z);
        m.put("on_ground", p.isOnGround());
        m.put("in_water", p.isTouchingWater());
        m.put("eye_height", (double) p.getStandingEyeHeight());
        m.put("health", p.getHealth());
        m.put("food", p.getHungerManager().getFoodLevel());
        m.put("dimension", w.getRegistryKey().getValue().toString());
        m.put("slot", p.getInventory().selectedSlot);
        m.put("attack_strength", p.getAttackCooldownProgress(HALF_TICK));
        return m;
    }

    private static boolean flag(JsonObject a, String key) {
        return a.has(key) && a.get(key).getAsBoolean();
    }

    private static float number(JsonObject a, String key) {
        return a.has(key) && !a.get(key).isJsonNull() ? a.get(key).getAsFloat() : 0f;
    }

    private static Object drive(GameOptions o, JsonObject a) {
        float forward = number(a, "forward");
        float strafe = number(a, "strafe");
        o.forwardKey.setPressed(forward > 0);
        o.backKey.setPressed(forward < 0);
        o.leftKey.setPressed(strafe > 0);
        o.rightKey.setPressed(strafe < 0);
        o.jumpKey.setPressed(flag(a, "jump"));
        o.sneakKey.setPressed(flag(a, "sneak"));
        o.sprintKey.setPressed(flag(a, "sprint"));
        return true;
    }

    private static Object release(GameOptions o) {
        for (var k : new net.minecraft.client.option.KeyBinding[] {o.forwardKey, o.backKey, o.leftKey, o.rightKey, o.jumpKey, o.sneakKey, o.sprintKey}) {
            k.setPressed(false);
        }
        // The keys the player really holds, back.
        net.minecraft.client.option.KeyBinding.updatePressedStates();
        return true;
    }

    private static Object look(ClientPlayerEntity p, JsonObject a) {
        p.setYaw(number(a, "yaw"));
        p.setPitch(Math.max(-90f, Math.min(90f, number(a, "pitch"))));
        return true;
    }

    private static Entity entity(ClientWorld w, String uuid) {
        for (Entity e : w.getEntities()) {
            if (e.getUuidAsString().equals(uuid)) return e;
        }
        return null;
    }

    private static Object attack(MinecraftClient mc, ClientPlayerEntity p, ClientWorld w, JsonObject a) {
        Entity target = a.has("entity") && !a.get("entity").isJsonNull() ? entity(w, a.get("entity").getAsString())
            : mc.crosshairTarget instanceof EntityHitResult hit ? hit.getEntity() : null;
        if (target != null) mc.interactionManager.attackEntity(p, target);
        p.swingHand(Hand.MAIN_HAND);
        return target != null;
    }

    private static BlockPos pos(JsonObject a) {
        return new BlockPos(a.get("x").getAsInt(), a.get("y").getAsInt(), a.get("z").getAsInt());
    }

    private static Direction face(JsonObject a) {
        Direction d = a.has("face") ? Direction.byName(a.get("face").getAsString()) : null;
        return d == null ? Direction.UP : d;
    }

    private static Object mine(MinecraftClient mc, ClientPlayerEntity p, ClientWorld w, JsonObject a) {
        if (flag(a, "stop")) {
            mc.interactionManager.cancelBlockBreaking();
            return false;
        }
        BlockPos at = pos(a);
        mc.interactionManager.updateBlockBreakingProgress(at, face(a));
        p.swingHand(Hand.MAIN_HAND);
        return !w.getBlockState(at).isAir();
    }

    private static Object place(MinecraftClient mc, ClientPlayerEntity p, JsonObject a) {
        BlockPos at = pos(a);
        Direction d = face(a);
        Vec3d hitAt = Vec3d.ofCenter(at).add(Vec3d.of(d.getVector()).multiply(FACE_OFFSET));
        boolean placed = mc.interactionManager.interactBlock(p, Hand.MAIN_HAND, new BlockHitResult(hitAt, d, at, false)).isAccepted();
        if (placed) p.swingHand(Hand.MAIN_HAND);
        return placed;
    }

    private static Object slot(ClientPlayerEntity p, JsonObject a) {
        int s = a.get("slot").getAsInt();
        if (s < 0 || s > 8) return false;
        p.getInventory().selectedSlot = s;
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

    private static boolean loaded(ClientWorld w, BlockPos at) {
        return w.getChunkManager().isChunkLoaded(at.getX() >> 4, at.getZ() >> 4);
    }

    private static Object scan(ClientWorld w, JsonObject a) {
        BlockPos[] b = box(a);
        if (b == null) return null;
        Set<Identifier> ids = new HashSet<>();
        Set<TagKey<Block>> tags = new HashSet<>();
        for (JsonElement e : a.getAsJsonArray("ids")) {
            String s = e.getAsString();
            Identifier id = Identifier.tryParse(s.startsWith(TAG_MARK) ? s.substring(TAG_MARK.length()) : s);
            if (id == null) continue;
            if (s.startsWith(TAG_MARK)) tags.add(TagKey.of(RegistryKeys.BLOCK, id));
            else ids.add(id);
        }
        JsonArray out = new JsonArray();
        for (BlockPos at : BlockPos.iterate(b[0], b[1])) {
            if (!loaded(w, at)) continue;
            BlockState st = w.getBlockState(at);
            Identifier id = Registries.BLOCK.getId(st.getBlock());
            boolean hit = ids.contains(id) || tags.stream().anyMatch(st::isIn);
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

    private static Object area(ClientWorld w, JsonObject a) {
        BlockPos[] b = box(a);
        if (b == null) return null;
        Map<BlockState, Integer> index = new HashMap<>();
        JsonArray palette = new JsonArray();
        JsonArray blocks = new JsonArray();
        for (int y = b[0].getY(); y <= b[1].getY(); y++)
            for (int z = b[0].getZ(); z <= b[1].getZ(); z++)
                for (int x = b[0].getX(); x <= b[1].getX(); x++) {
                    BlockPos at = new BlockPos(x, y, z);
                    BlockState st = loaded(w, at) ? w.getBlockState(at) : Blocks.BEDROCK.getDefaultState();
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

    private static JsonObject kind(ClientWorld w, BlockPos at, BlockState st) {
        JsonObject k = new JsonObject();
        k.addProperty("id", Registries.BLOCK.getId(st.getBlock()).toString());
        k.addProperty("solid", !st.getCollisionShape(w, at).isEmpty());
        k.addProperty("liquid", !st.getFluidState().isEmpty());
        k.addProperty("climbable", st.isIn(BlockTags.CLIMBABLE));
        k.addProperty("harmful", st.isIn(BlockTags.FIRE) || st.getFluidState().isIn(FluidTags.LAVA) || HARMFUL.contains(st.getBlock()));
        return k;
    }

    private static Object entities(ClientPlayerEntity p, ClientWorld w, JsonObject a) {
        double r = a.has("radius") ? a.get("radius").getAsDouble() : 0;
        JsonArray out = new JsonArray();
        for (Entity e : w.getEntities()) {
            if (e == p || e.squaredDistanceTo(p) > r * r) continue;
            JsonObject o = new JsonObject();
            o.addProperty("uuid", e.getUuidAsString());
            o.addProperty("type", Registries.ENTITY_TYPE.getId(e.getType()).toString());
            o.addProperty("x", e.getX());
            o.addProperty("y", e.getY());
            o.addProperty("z", e.getZ());
            o.addProperty("eye_height", (double) e.getStandingEyeHeight());
            if (e instanceof LivingEntity l) o.addProperty("health", l.getHealth());
            if (e instanceof PlayerEntity pl) o.addProperty("name", pl.getGameProfile().getName());
            o.addProperty("hostile", e instanceof Monster);
            out.add(o);
        }
        return out;
    }

    private static Object inventory(ClientPlayerEntity p) {
        JsonArray out = new JsonArray();
        var inv = p.getInventory();
        for (int i = 0; i < inv.size(); i++) {
            ItemStack st = inv.getStack(i);
            if (st.isEmpty()) continue;
            JsonObject o = new JsonObject();
            o.addProperty("slot", i);
            o.addProperty("item", Registries.ITEM.getId(st.getItem()).toString());
            o.addProperty("count", st.getCount());
            out.add(o);
        }
        return out;
    }

    private static Object click(MinecraftClient mc, ClientPlayerEntity p, JsonObject a) {
        SlotActionType mode = SlotActionType.valueOf(a.get("mode").getAsString().toUpperCase(java.util.Locale.ROOT));
        mc.interactionManager.clickSlot(p.currentScreenHandler.syncId, a.get("slot").getAsInt(), a.get("button").getAsInt(), mode, p);
        return true;
    }

    private static Object chat(MinecraftClient mc, JsonObject a) {
        String text = a.get("text").getAsString();
        if (text.startsWith("/")) mc.getNetworkHandler().sendChatCommand(text.substring(1));
        else mc.getNetworkHandler().sendChatMessage(text);
        return true;
    }
}
