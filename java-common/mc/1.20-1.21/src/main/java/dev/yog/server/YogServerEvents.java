package dev.yog.server;

import dev.yog.NativeBridge;
import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.actions.YogNetActions;
import dev.yog.content.blocks.YogInventoryBlockEntity;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.LevelResource;
import net.minecraft.world.phys.BlockHitResult;

/**
 * What the server's events mean to the mods, the same on every loader: a
 * loader's host hooks its own events up to these. A {@code boolean} answer
 * is whether the thing may go on (false: cancel it).
 */
public final class YogServerEvents {
    private YogServerEvents() {}

    /** The channel entity data goes on (yog-runtime's). */
    /** Ticks a joining player is waited for in the server's list before giving up. */
    private static final int JOIN_TICKS = 40;
    /** The hands, as the mods name them. */
    private static final String MAIN_HAND = "main_hand", OFF_HAND = "off_hand";

    /** Players who joined, waited for till they're in the server's list: UUID → ticks left. */
    private static final Map<UUID, Integer> PENDING_JOINS = new ConcurrentHashMap<>();

    private static String dim(Level level) {
        return level.dimension().location().toString();
    }

    private static String name(Player p) {
        return p.getName().getString();
    }

    private static String blockId(BlockState state) {
        return BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
    }

    private static String typeId(Entity e) {
        return BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString();
    }

    public static void serverStarted(MinecraftServer server) {
        NativeBridge.setServer(server);
        new YogEvents.Server.Life.Started(server.getWorldPath(LevelResource.ROOT).toAbsolutePath().toString()).emit();
    }

    public static void serverStopping() {
        new YogEvents.Server.Life.Stopping().emit();
    }

    /** The end of a server tick: joined players who are in the list now are announced, then the mods tick. */
    public static void serverTick(MinecraftServer server) {
        if (!PENDING_JOINS.isEmpty()) {
            List<UUID> done = new ArrayList<>();
            PENDING_JOINS.forEach((uuid, left) -> {
                ServerPlayer found = server.getPlayerList().getPlayer(uuid);
                if (found != null) {
                    new YogEvents.Player.Session.Join(name(found), uuid.toString(), dim(found.level())).emit();
                    done.add(uuid);
                } else if (left <= 0) {
                    YogLog.warn("player join: " + uuid + " never appeared in the server's list");
                    done.add(uuid);
                } else {
                    PENDING_JOINS.put(uuid, left - 1);
                }
            });
            done.forEach(PENDING_JOINS::remove);
        }
        new YogEvents.Server.Game.Tick().emit();
    }

    /** A player connected: announced once the server lists them; their entity data sent. */
    public static void playerJoined(ServerPlayer p) {
        PENDING_JOINS.put(p.getUUID(), JOIN_TICKS);
        sendEntityData(p, p);
    }

    public static void playerLeft(ServerPlayer p) {
        PENDING_JOINS.remove(p.getUUID());
        new YogEvents.Player.Session.Leave(name(p), p.getStringUUID(), dim(p.level())).emit();
    }

    public static boolean blockBreakPre(Level level, Player player, BlockPos pos, BlockState state) {
        return new YogEvents.Block.Break(name(player), blockId(state), new YogTypes.BlockPos(pos.getX(), pos.getY(), pos.getZ()), dim(level)).emitPre();
    }

    /** A block broken: to the mods; an inventory's contents dropped. */
    public static void blockBroken(Level level, Player player, BlockPos pos, BlockState state, BlockEntity blockEntity) {
        new YogEvents.Block.Break(name(player), blockId(state), new YogTypes.BlockPos(pos.getX(), pos.getY(), pos.getZ()), dim(level)).emit();
        if (blockEntity instanceof YogInventoryBlockEntity inv && !inv.isEmpty()) {
            net.minecraft.world.Containers.dropContents(level, pos, inv);
            inv.clearContent();
        }
    }

    public static boolean chatPre(ServerPlayer p, String message) {
        return new YogEvents.Player.Chat.Message(name(p), message, dim(p.level())).emitPre();
    }

    public static void chat(ServerPlayer p, String message) {
        new YogEvents.Player.Chat.Message(name(p), message, dim(p.level())).emit();
    }

    /** An item used (right-click); as vanilla, one cooling down isn't. */
    public static boolean useItem(ServerPlayer sp, InteractionHand hand) {
        ItemStack stack = sp.getItemInHand(hand);
        if (sp.getCooldowns().isOnCooldown(stack.getItem())) return true;
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        String dim = dim(sp.level());
        if (!new YogEvents.Player.Action.UseItem(name(sp), itemId, sp.isShiftKeyDown(), dim, hand == InteractionHand.OFF_HAND).emitPre()) return false;
        new YogEvents.Player.Action.UseItem(name(sp), itemId, sp.isShiftKeyDown(), dim, hand == InteractionHand.OFF_HAND).emit();
        return true;
    }

    /**
     * A block used (right-click on it), and a block item placed by it. As
     * vanilla, sneaking with something in hand uses the item, not the block.
     */
    public static boolean useBlock(ServerPlayer sp, InteractionHand hand, BlockHitResult hit) {
        Level level = sp.level();
        String dim = dim(level);
        boolean itemFirst = sp.isSecondaryUseActive() && (!sp.getMainHandItem().isEmpty() || !sp.getOffhandItem().isEmpty());
        if (!itemFirst) {
            BlockPos pos = hit.getBlockPos();
            String blockId = blockId(level.getBlockState(pos));
            if (!new YogEvents.Block.Use(name(sp), blockId, new YogTypes.BlockPos(pos.getX(), pos.getY(), pos.getZ()), dim, sp.isShiftKeyDown()).emitPre()) return false;
            new YogEvents.Block.Use(name(sp), blockId, new YogTypes.BlockPos(pos.getX(), pos.getY(), pos.getZ()), dim, sp.isShiftKeyDown()).emit();
        }
        if (sp.getItemInHand(hand).getItem() instanceof BlockItem bi) {
            BlockPos placed = hit.getBlockPos().relative(hit.getDirection());
            String blockId = BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
            return new YogEvents.Block.Place(name(sp), blockId, new YogTypes.BlockPos(placed.getX(), placed.getY(), placed.getZ()), dim, sp.isShiftKeyDown()).emitPre();
        }
        return true;
    }

    public static boolean interactEntity(ServerPlayer sp, InteractionHand hand, Entity entity) {
        String handName = hand == InteractionHand.MAIN_HAND ? MAIN_HAND : OFF_HAND;
        String dim = dim(sp.level());
        if (!new YogEvents.Player.Action.Interact(name(sp), sp.getStringUUID(), typeId(entity), entity.getStringUUID(), handName, dim).emitPre()) return false;
        new YogEvents.Player.Action.Interact(name(sp), sp.getStringUUID(), typeId(entity), entity.getStringUUID(), handName, dim).emit();
        return true;
    }

    public static void attackEntity(ServerPlayer sp, Entity entity) {
        new YogEvents.Player.Action.Attack(name(sp), typeId(entity), entity.getStringUUID(), dim(sp.level())).emit();
    }

    public static boolean damage(LivingEntity entity, DamageSource source, float amount) {
        String type = typeId(entity), dim = dim(entity.level());
        if (!new YogEvents.Entity.Combat.Damage(type, entity.getStringUUID(), amount, source.getMsgId(), dim).emitPre()) return false;
        new YogEvents.Entity.Combat.Damage(type, entity.getStringUUID(), amount, source.getMsgId(), dim).emit();
        return true;
    }

    /** An entity came into a server world: whether it stays. */
    public static boolean entityLoaded(Entity entity, Level level) {
        String type = typeId(entity), uuid = entity.getStringUUID(), dim = dim(level);
        if (!new YogEvents.Entity.Life.Spawn(type, uuid, dim).emitPre()) return false;
        new YogEvents.Entity.Life.Spawn(type, uuid, dim).emit();
        return true;
    }

    /** An entity left the server's memory: its data is forgotten (what it keeps was saved with it). */
    public static void entityUnloaded(Entity entity) {
        if (!(entity instanceof Player)) NativeBridge.nativeEntityDataForget(entity.getStringUUID());
    }

    /** A living thing died; a player's death has its own events ({@link #playerDeathPre}). */
    public static void died(LivingEntity entity, DamageSource source) {
        if (entity instanceof ServerPlayer) return;
        new YogEvents.Entity.Life.Death(typeId(entity), entity.getStringUUID(), source.getMsgId(), dim(entity.level())).emit();
    }

    public static boolean playerDeathPre(ServerPlayer sp, DamageSource source) {
        String dim = dim(sp.level());
        if (!new YogEvents.Player.Life.Death(name(sp), sp.getStringUUID(), source.getMsgId(), dim).emitPre()) return false;
        new YogEvents.Player.Life.Death(name(sp), sp.getStringUUID(), source.getMsgId(), dim).emit();
        return true;
    }

    /** A player back: after death to the mods ({@code alive}: back from the End, not from death); their data sent again. */
    public static void respawned(ServerPlayer p, boolean alive) {
        sendEntityData(p, p);
        if (!alive) new YogEvents.Player.Life.Respawn(name(p), p.getStringUUID(), false, dim(p.level())).emit();
    }

    /** A player went to another world: their data sent again. */
    public static void changedLevel(ServerPlayer p) {
        sendEntityData(p, p);
    }

    /** {@code player} started seeing {@code entity}: all its data. */
    public static void startedTracking(Entity entity, ServerPlayer player) {
        sendEntityData(player, entity);
    }

    /** Everything entity {@code about} keeps, to {@code to}. */
    private static void sendEntityData(ServerPlayer to, Entity about) {
        byte[] data = NativeBridge.nativeEntityDataFor(about.getStringUUID());
        if (data != null) YogNetActions.sendToPlayer(name(to), YogNames.Yog.ENTITY_DATA, data);
    }
}
