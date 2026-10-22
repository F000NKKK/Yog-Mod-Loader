package dev.yog;

import net.minecraft.world.phys.HitResult;

import net.minecraft.world.level.ClipContext;

import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.function.Predicate;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.TagKey;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.ClickType;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * This client's player for the bus ({@code yog_event::client::local} and
 * {@code client::control}): what it is and sees, as {@link YogTypes}
 * records, and doing what a player does — its own keys held, so the game
 * sends the server what a player's input would. Called on the client
 * thread; without a player each answers none ({@code null}, {@code false}).
 */
public final class YogClientOps {
    private YogClientOps() {}

    /** At most this many blocks are sent in one area. */
    private static final long MAX_BLOCKS = 1L << 21;
    /**
     * At most this many chunk columns are scanned in one call: a scan goes
     * chunk by chunk (what isn't loaded costs nothing, a section without the
     * blocks one look), so its box's volume isn't what it costs.
     */
    private static final long MAX_SCAN_COLUMNS = 1L << 14;
    /** Blocks along a chunk section's edge, and its shift. */
    private static final int SECTION = 16;
    private static final int SECTION_SHIFT = 4;
    /** A face's hit point: this far from the block's centre toward the face. */
    private static final double FACE_OFFSET = 0.5;
    /** The partial tick the attack's charge is read at, as the game's crosshair does. */
    private static final float HALF_TICK = 0.5f;
    /** What a tag reference starts with, in a scan's ids. */
    private static final String TAG_MARK = "#";
    /** How far the player's head turns up or down. */
    private static final float MAX_PITCH = 90f;
    /** The hotbar's slots. */
    private static final int HOTBAR = 9;
    /** What a chat line that's a command starts with. */
    private static final String COMMAND_MARK = "/";

    private static Minecraft mc() {
        return Minecraft.getInstance();
    }

    // ── what it is and sees ──────────────────────────────────────────────────

    @YogServiceHost
    public static YogTypes.LocalState state() {
        LocalPlayer p = mc().player;
        ClientLevel w = mc().level;
        if (p == null || w == null) return null;
        return new YogTypes.LocalState(YogQueries.point(p.position()), p.getYRot(), p.getXRot(), YogQueries.point(p.getDeltaMovement()),
                p.onGround(), p.isInWater(), p.getEyeHeight(), p.getHealth(), p.getFoodData().getFoodLevel(),
                w.dimension().location().toString(), p.getInventory().selected, p.getAttackStrengthScale(HALF_TICK), p.isCrouching());
    }

    @YogServiceHost
    public static YogTypes.BlockHit raycast(double fx, double fy, double fz, double tx, double ty, double tz) {
        LocalPlayer p = mc().player;
        ClientLevel w = mc().level;
        if (p == null || w == null) return null;
        BlockHitResult hit = w.clip(new ClipContext(new Vec3(fx, fy, fz), new Vec3(tx, ty, tz),
                ClipContext.Block.OUTLINE, ClipContext.Fluid.NONE, p));
        return blockHit(hit);
    }

    @YogServiceHost
    public static YogTypes.BlockHit target() {
        return mc().hitResult instanceof BlockHitResult hit ? blockHit(hit) : null;
    }

    private static YogTypes.BlockHit blockHit(BlockHitResult hit) {
        if (hit == null || hit.getType() != HitResult.Type.BLOCK) return null;
        return new YogTypes.BlockHit(YogQueries.blockPos(hit.getBlockPos()), hit.getDirection().getName(), YogQueries.point(hit.getLocation()));
    }

    @YogServiceHost
    public static List<YogTypes.SlotStack> inventory() {
        LocalPlayer p = mc().player;
        if (p == null) return List.of();
        var inv = p.getInventory();
        List<YogTypes.SlotStack> out = new ArrayList<>();
        for (int i = 0; i < inv.getContainerSize(); i++) {
            YogTypes.Stack s = YogQueries.stack(inv.getItem(i));
            if (s != null) out.add(new YogTypes.SlotStack(i, s));
        }
        return out;
    }

    /** The box between two corners, each the lesser and the greater. */
    private static BlockPos[] ordered(BlockPos a, BlockPos b) {
        return new BlockPos[] {
                new BlockPos(Math.min(a.getX(), b.getX()), Math.min(a.getY(), b.getY()), Math.min(a.getZ(), b.getZ())),
                new BlockPos(Math.max(a.getX(), b.getX()), Math.max(a.getY(), b.getY()), Math.max(a.getZ(), b.getZ()))};
    }

    /**
     * The blocks of {@code ids} (ids and {@code #tags}) in the box: chunk by
     * chunk, a section whose palette has none of them skipped whole (most
     * of a box is stone and air, the ores a few sections hold).
     */
    @YogServiceHost
    public static List<YogTypes.PlacedBlock> scan(int x1, int y1, int z1, int x2, int y2, int z2, List<String> ids) {
        ClientLevel w = mc().level;
        if (w == null) return List.of();
        BlockPos[] b = ordered(new BlockPos(x1, y1, z1), new BlockPos(x2, y2, z2));
        long columns = (long) ((b[1].getX() >> SECTION_SHIFT) - (b[0].getX() >> SECTION_SHIFT) + 1) * ((b[1].getZ() >> SECTION_SHIFT) - (b[0].getZ() >> SECTION_SHIFT) + 1);
        if (columns > MAX_SCAN_COLUMNS) return List.of();
        Predicate<BlockState> wanted = wanted(ids);
        List<YogTypes.PlacedBlock> out = new ArrayList<>();
        for (int cx = b[0].getX() >> SECTION_SHIFT; cx <= b[1].getX() >> SECTION_SHIFT; cx++) {
            for (int cz = b[0].getZ() >> SECTION_SHIFT; cz <= b[1].getZ() >> SECTION_SHIFT; cz++) {
                LevelChunk chunk = w.getChunkSource().getChunkNow(cx, cz);
                if (chunk == null) continue;
                LevelChunkSection[] sections = chunk.getSections();
                for (int i = 0; i < sections.length; i++) {
                    int y0 = chunk.getSectionYFromSectionIndex(i) << SECTION_SHIFT;
                    if (y0 + SECTION <= b[0].getY() || y0 > b[1].getY()) continue;
                    if (sections[i].hasOnlyAir() || !sections[i].maybeHas(wanted)) continue;
                    scanSection(sections[i], new BlockPos(cx << SECTION_SHIFT, y0, cz << SECTION_SHIFT), b, wanted, out);
                }
            }
        }
        return out;
    }

    /** Whether a state is one of {@code ids} (block ids, {@code #tags}). */
    private static Predicate<BlockState> wanted(List<String> ids) {
        Set<Block> blocks = new HashSet<>();
        Set<TagKey<Block>> tags = new HashSet<>();
        for (String s : ids) {
            boolean tag = s.startsWith(TAG_MARK);
            ResourceLocation id = ResourceLocation.tryParse(tag ? s.substring(TAG_MARK.length()) : s);
            if (id == null) continue;
            if (tag) tags.add(TagKey.create(Registries.BLOCK, id));
            else BuiltInRegistries.BLOCK.getOptional(id).ifPresent(blocks::add);
        }
        return st -> blocks.contains(st.getBlock()) || tags.stream().anyMatch(st::is);
    }

    /** The wanted blocks of one section (its corner {@code at}) inside the box. */
    private static void scanSection(LevelChunkSection section, BlockPos at, BlockPos[] b, Predicate<BlockState> wanted, List<YogTypes.PlacedBlock> out) {
        int x0 = at.getX(), y0 = at.getY(), z0 = at.getZ();
        for (int ly = Math.max(0, b[0].getY() - y0); ly < SECTION && y0 + ly <= b[1].getY(); ly++) {
            for (int lz = Math.max(0, b[0].getZ() - z0); lz < SECTION && z0 + lz <= b[1].getZ(); lz++) {
                for (int lx = Math.max(0, b[0].getX() - x0); lx < SECTION && x0 + lx <= b[1].getX(); lx++) {
                    BlockState st = section.getBlockState(lx, ly, lz);
                    if (wanted.test(st)) out.add(new YogTypes.PlacedBlock(new YogTypes.BlockPos(x0 + lx, y0 + ly, z0 + lz), BlockStateParser.serialize(st)));
                }
            }
        }
    }

    @YogServiceHost
    public static YogTypes.BlockArea area(int x1, int y1, int z1, int x2, int y2, int z2) {
        ClientLevel w = mc().level;
        if (w == null) return null;
        BlockPos[] b = ordered(new BlockPos(x1, y1, z1), new BlockPos(x2, y2, z2));
        long volume = (long) (b[1].getX() - b[0].getX() + 1) * (b[1].getY() - b[0].getY() + 1) * (b[1].getZ() - b[0].getZ() + 1);
        if (volume > MAX_BLOCKS) return null;
        Map<BlockState, Integer> index = new HashMap<>();
        List<YogTypes.BlockKind> palette = new ArrayList<>();
        int[] blocks = new int[(int) volume];
        int next = 0;
        for (int y = b[0].getY(); y <= b[1].getY(); y++)
            for (int z = b[0].getZ(); z <= b[1].getZ(); z++)
                for (int x = b[0].getX(); x <= b[1].getX(); x++) {
                    BlockPos at = new BlockPos(x, y, z);
                    BlockState st = w.getChunkSource().hasChunk(x >> SECTION_SHIFT, z >> SECTION_SHIFT) ? w.getBlockState(at) : Blocks.BEDROCK.defaultBlockState();
                    blocks[next++] = index.computeIfAbsent(st, s -> {
                        palette.add(kind(w, at, s));
                        return palette.size() - 1;
                    });
                }
        return new YogTypes.BlockArea(YogQueries.blockPos(b[0]), YogQueries.blockPos(b[1]), palette, blocks);
    }

    private static YogTypes.BlockKind kind(ClientLevel w, BlockPos at, BlockState st) {
        return new YogTypes.BlockKind(BuiltInRegistries.BLOCK.getKey(st.getBlock()).toString(), YogBlockTraits.solid(w, at, st),
                YogBlockTraits.liquid(st), YogBlockTraits.climbable(st), YogBlockTraits.harmful(st));
    }

    @YogServiceHost
    public static List<YogTypes.NearbyEntity> entities(double radius) {
        LocalPlayer p = mc().player;
        ClientLevel w = mc().level;
        if (p == null || w == null) return List.of();
        List<YogTypes.NearbyEntity> out = new ArrayList<>();
        for (Entity e : w.entitiesForRendering()) {
            if (e == p || e.distanceToSqr(p) > radius * radius) continue;
            Float health = e instanceof LivingEntity l ? l.getHealth() : null;
            String name = e instanceof Player pl ? pl.getGameProfile().getName() : null;
            out.add(new YogTypes.NearbyEntity(e.getStringUUID(), BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString(),
                    YogQueries.point(e.position()), e.getEyeHeight(), YogQueries.point(e.getDeltaMovement()), health, name, e instanceof Enemy));
        }
        return out;
    }

    // ── what it does ─────────────────────────────────────────────────────────

    @YogServiceHost
    public static boolean drive(float forward, float strafe, boolean jump, boolean sneak, boolean sprint, boolean useItem) {
        if (mc().player == null) return false;
        Options o = mc().options;
        o.keyUp.setDown(forward > 0);
        o.keyDown.setDown(forward < 0);
        o.keyLeft.setDown(strafe > 0);
        o.keyRight.setDown(strafe < 0);
        o.keyJump.setDown(jump);
        o.keyShift.setDown(sneak);
        o.keySprint.setDown(sprint);
        o.keyUse.setDown(useItem);
        return true;
    }

    @YogServiceHost
    public static boolean release() {
        Options o = mc().options;
        for (KeyMapping k : new KeyMapping[] {o.keyUp, o.keyDown, o.keyLeft, o.keyRight, o.keyJump, o.keyShift, o.keySprint, o.keyUse}) {
            k.setDown(false);
        }
        // The keys the player really holds, back.
        KeyMapping.setAll();
        return true;
    }

    @YogServiceHost
    public static boolean look(float yaw, float pitch) {
        LocalPlayer p = mc().player;
        if (p == null) return false;
        p.setYRot(yaw);
        p.setXRot(Math.max(-MAX_PITCH, Math.min(MAX_PITCH, pitch)));
        return true;
    }

    @YogServiceHost
    public static boolean attack(String uuid) {
        Minecraft mc = mc();
        LocalPlayer p = mc.player;
        if (p == null || mc.level == null) return false;
        Entity target = uuid != null ? find(mc.level, uuid) : mc.hitResult instanceof EntityHitResult hit ? hit.getEntity() : null;
        if (target != null) mc.gameMode.attack(p, target);
        p.swing(InteractionHand.MAIN_HAND);
        return target != null;
    }

    private static Entity find(ClientLevel w, String uuid) {
        for (Entity e : w.entitiesForRendering()) {
            if (e.getStringUUID().equals(uuid)) return e;
        }
        return null;
    }

    @YogServiceHost
    public static boolean useItem(boolean offHand) {
        LocalPlayer p = mc().player;
        return p != null && mc().gameMode.useItem(p, offHand ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND).consumesAction();
    }

    private static Direction face(String face) {
        Direction d = Direction.byName(face);
        return d == null ? Direction.UP : d;
    }

    @YogServiceHost
    public static boolean mine(int x, int y, int z, String face) {
        LocalPlayer p = mc().player;
        if (p == null || mc().level == null) return false;
        BlockPos at = new BlockPos(x, y, z);
        mc().gameMode.continueDestroyBlock(at, face(face));
        p.swing(InteractionHand.MAIN_HAND);
        return !mc().level.getBlockState(at).isAir();
    }

    @YogServiceHost
    public static boolean stopMining() {
        if (mc().player == null) return false;
        mc().gameMode.stopDestroyBlock();
        return true;
    }

    @YogServiceHost
    public static boolean place(int x, int y, int z, String face) {
        LocalPlayer p = mc().player;
        if (p == null) return false;
        BlockPos at = new BlockPos(x, y, z);
        Direction d = face(face);
        Vec3 hitAt = Vec3.atCenterOf(at).add(Vec3.atLowerCornerOf(d.getNormal()).scale(FACE_OFFSET));
        boolean placed = mc().gameMode.useItemOn(p, InteractionHand.MAIN_HAND, new BlockHitResult(hitAt, d, at, false)).consumesAction();
        if (placed) p.swing(InteractionHand.MAIN_HAND);
        return placed;
    }

    @YogServiceHost
    public static boolean selectSlot(int slot) {
        LocalPlayer p = mc().player;
        if (p == null || slot < 0 || slot >= HOTBAR) return false;
        p.getInventory().selected = slot;
        return true;
    }

    @YogServiceHost
    public static boolean click(int slot, int button, String mode) {
        LocalPlayer p = mc().player;
        if (p == null) return false;
        ClickType input = ClickType.valueOf(mode.toUpperCase(Locale.ROOT));
        mc().gameMode.handleInventoryMouseClick(p.containerMenu.containerId, slot, button, input, p);
        return true;
    }

    @YogServiceHost
    public static boolean chat(String text) {
        var connection = mc().getConnection();
        if (connection == null) return false;
        if (text.startsWith(COMMAND_MARK)) connection.sendCommand(text.substring(COMMAND_MARK.length()));
        else connection.sendChat(text);
        return true;
    }
}
