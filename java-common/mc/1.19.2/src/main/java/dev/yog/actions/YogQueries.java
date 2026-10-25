package dev.yog.actions;

import net.minecraft.core.Registry;
import dev.yog.NativeBridge;
import dev.yog.YogLog;
import dev.yog.YogServiceHost;
import dev.yog.content.blocks.YogInventoryMenu;

import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.Container;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.block.RedStoneWireBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;

/**
 * The world's queries the bus asks the host ({@code yog_event}'s entity,
 * block, player, item and world actions): each answers with the record
 * {@link YogTypes} has for its reply, or {@code null} for none.
 */
public final class YogQueries {
    private YogQueries() {}

    /** Block flags {@link ServerLevel#setBlock} is called with: notify neighbours and clients. */
    private static final int SET_BLOCK_FLAGS = 2;

    // ── conversions ──────────────────────────────────────────────────────────

    public static YogTypes.Point point(Vec3 v) {
        return new YogTypes.Point(v.x, v.y, v.z);
    }

    public static YogTypes.BlockPos blockPos(BlockPos p) {
        return new YogTypes.BlockPos(p.getX(), p.getY(), p.getZ());
    }

    /** A stack as the bus carries it; {@code null} for an empty one. */
    public static YogTypes.Stack stack(ItemStack s) {
        if (s.isEmpty()) return null;
        return new YogTypes.Stack(Registry.ITEM.getKey(s.getItem()).toString(), s.getCount(), NativeBridge.stackSnbt(s));
    }

    // ── entities ─────────────────────────────────────────────────────────────

    /** The id of the dimension {@code e} is in. */
    public static String dimensionId(Entity e) {
        return e.getLevel().dimension().location().toString();
    }

    @YogServiceHost
    public static YogTypes.Point entityPosition(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        return e == null ? null : point(e.position());
    }

    @YogServiceHost
    public static YogTypes.Rotation entityRotation(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        return e == null ? null : new YogTypes.Rotation(e.getYRot(), e.getXRot());
    }

    @YogServiceHost
    public static YogTypes.Point entityVelocity(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        return e == null ? null : point(e.getDeltaMovement());
    }

    @YogServiceHost
    public static YogTypes.EntityInfo entityInfo(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        if (e == null) return null;
        return new YogTypes.EntityInfo(
                Registry.ENTITY_TYPE.getKey(e.getType()).toString(),
                dimensionId(e),
                point(e.position()), e.getEyeHeight(), e.getBbWidth(), e.getBbHeight(),
                NativeBridge.entityFlags(e), e.getName().getString());
    }

    @YogServiceHost
    public static List<String> entityPassengers(String uuid) {
        Entity e = NativeBridge.entityByUuid(uuid);
        return e == null ? List.of() : e.getPassengers().stream().map(Entity::getStringUUID).toList();
    }

    @YogServiceHost
    public static YogTypes.Stack itemEntityStack(String uuid) {
        return NativeBridge.entityByUuid(uuid) instanceof ItemEntity ie ? stack(ie.getItem()) : null;
    }

    // ── players ──────────────────────────────────────────────────────────────

    @YogServiceHost
    public static YogTypes.PlayerInfo playerInfo(String player) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return null;
        return new YogTypes.PlayerInfo(p.getStringUUID(), p.gameMode.getGameModeForPlayer().getName(),
                p.getInventory().selected, p.experienceLevel, p.totalExperience, p.getAirSupply(),
                p.getAbilities().mayfly, p.getAbilities().flying);
    }

    @YogServiceHost
    public static List<YogTypes.SlotStack> playerInventory(String player) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return List.of();
        Inventory inv = p.getInventory();
        List<YogTypes.SlotStack> out = new ArrayList<>();
        for (int i = 0; i < inv.getContainerSize(); i++) {
            YogTypes.Stack s = stack(inv.getItem(i));
            if (s != null) out.add(new YogTypes.SlotStack(i, s));
        }
        return out;
    }

    @YogServiceHost
    public static YogTypes.Stack getSlotItem(String player, int slot) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return null;
        Inventory inv = p.getInventory();
        return slot < 0 || slot >= inv.getContainerSize() ? null : stack(inv.getItem(slot));
    }

    @YogServiceHost
    public static List<String> onlinePlayers() {
        MinecraftServer s = NativeBridge.server;
        return s == null ? List.of() : s.getPlayerList().getPlayers().stream().map(p -> p.getName().getString()).toList();
    }

    // ── this client ──────────────────────────────────────────────────────────

    /** The slots of the inventory screen open on this client: what each holds, where it's drawn. */
    @YogServiceHost
    public static List<YogTypes.ScreenSlot> screenSlots() {
        YogInventoryMenu menu = NativeBridge.activeInventoryMenu;
        if (menu == null) return List.of();
        return menu.slots.stream().map(slot -> new YogTypes.ScreenSlot(stack(slot.getItem()), slot.x, slot.y)).toList();
    }

    // ── blocks ───────────────────────────────────────────────────────────────

    @YogServiceHost
    public static YogTypes.BlockInfo blockInfo(String dim, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return null;
        BlockPos pos = new BlockPos(x, y, z);
        BlockState st = w.getBlockState(pos);
        FluidState fs = st.getFluidState();
        String fluid = fs.isEmpty() ? null : Registry.FLUID.getKey(fs.getType()).toString();
        boolean inWorld = w.isInWorldBounds(pos) && w.getWorldBorder().isWithinBounds(pos);
        return new YogTypes.BlockInfo(BlockStateParser.serialize(st), st.getDestroySpeed(w, pos), st.canBeReplaced(), st.isAir(),
                fluid, !fs.isEmpty() && fs.isSource(), w.getBestNeighborSignal(pos), inWorld);
    }

    @YogServiceHost
    public static YogTypes.Stack getInventorySlot(String dim, int x, int y, int z, int slot) {
        Container inv = NativeBridge.containerAt(dim, x, y, z);
        return inv == null || slot < 0 || slot >= inv.getContainerSize() ? null : stack(inv.getItem(slot));
    }

    @YogServiceHost
    public static Integer redstonePower(String dim, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dim);
        return w == null ? null : w.getBestNeighborSignal(new BlockPos(x, y, z));
    }

    @YogServiceHost
    public static Integer redstoneSignal(String dim, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return null;
        BlockPos pos = new BlockPos(x, y, z);
        BlockState state = w.getBlockState(pos);
        if (state.getBlock() instanceof RedStoneWireBlock) return state.getValue(RedStoneWireBlock.POWER);
        if (!state.isSignalSource()) return null;
        int best = 0;
        for (Direction d : Direction.values()) best = Math.max(best, state.getSignal(w, pos, d));
        return best;
    }

    // ── the world ────────────────────────────────────────────────────────────

    @YogServiceHost
    public static List<YogTypes.PlacedBlock> scanRegion(String dim, int x1, int y1, int z1, int x2, int y2, int z2) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return List.of();
        List<YogTypes.PlacedBlock> out = new ArrayList<>();
        for (BlockPos pos : BlockPos.betweenClosed(new BlockPos(x1, y1, z1), new BlockPos(x2, y2, z2))) {
            BlockState st = w.getBlockState(pos);
            if (!st.isAir()) out.add(new YogTypes.PlacedBlock(blockPos(pos), BlockStateParser.serialize(st)));
        }
        return out;
    }

    @YogServiceHost
    public static Integer fillRegion(String dim, int x1, int y1, int z1, int x2, int y2, int z2, String state) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return null;
        try {
            BlockState parsed = BlockStateParser.parseForBlock(Registry.BLOCK.asLookup(), state, false).blockState();
            int changed = 0;
            for (BlockPos pos : BlockPos.betweenClosed(new BlockPos(x1, y1, z1), new BlockPos(x2, y2, z2))) {
                if (w.setBlock(pos, parsed, SET_BLOCK_FLAGS)) changed++;
            }
            return changed;
        } catch (Exception e) {
            YogLog.warn("fillRegion: " + state + ": " + e.getMessage());
            return null;
        }
    }

    @YogServiceHost
    public static Integer worldEntityCount(String dim, String entityTypeId) {
        ServerLevel w = NativeBridge.worldFor(dim);
        ResourceLocation id = ResourceLocation.tryParse(entityTypeId);
        if (w == null || id == null || !Registry.ENTITY_TYPE.containsKey(id)) return null;
        EntityType<?> type = Registry.ENTITY_TYPE.get(id);
        int count = 0;
        for (Entity e : w.getAllEntities()) if (e.getType() == type) count++;
        return count;
    }

    @YogServiceHost
    public static List<String> entitiesInBox(String dim, double x0, double y0, double z0, double x1, double y1, double z1) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return List.of();
        return w.getEntities((Entity) null, new AABB(x0, y0, z0, x1, y1, z1), o -> true).stream().map(Entity::getStringUUID).toList();
    }

    @YogServiceHost
    public static YogTypes.BlockHit raycastBlock(String dim, double fx, double fy, double fz, double tx, double ty, double tz) {
        ServerLevel w = NativeBridge.worldFor(dim);
        if (w == null) return null;
        BlockHitResult hit = w.clip(new ClipContext(new Vec3(fx, fy, fz), new Vec3(tx, ty, tz),
                ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, (Entity) null));
        if (hit == null || hit.getType() == HitResult.Type.MISS) return null;
        return new YogTypes.BlockHit(blockPos(hit.getBlockPos()), hit.getDirection().getName(), point(hit.getLocation()));
    }
}
