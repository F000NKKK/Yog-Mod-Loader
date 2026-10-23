package dev.yog;

import dev.yog.content.blocks.YogInventoryMenu;

import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.yog.bus.YogTypes;
import dev.yog.util.YogEntityFlags;
import dev.yog.util.YogNativeRuntime;
import java.util.Locale;
import java.util.UUID;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LevelEvent;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.level.storage.loot.LootDataId;
import net.minecraft.world.level.storage.loot.LootDataType;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.parameters.LootContextParamSets;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.ItemStack;
import net.minecraft.network.protocol.game.ClientboundSetActionBarTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.core.Registry;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundSource;
import net.minecraft.network.chat.Component;
import net.minecraft.core.BlockPos;
import org.lwjgl.glfw.GLFW;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.commands.arguments.blocks.BlockInput;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.Direction;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.BoneMealItem;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import java.util.Map;

/**
 * Bridge between the host (any loader: {@link YogPlatform}) and the native Yog runtime ({@code libyog_runtime}).
 * Calls into Rust go through the {@code native} methods; calls back from Rust into
 * the game are static methods invoked over JNI.
 */
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
public final class NativeBridge {
    private static boolean loaded = false;
    public static volatile MinecraftServer server;

    private NativeBridge() {}

    /** The running server, or null. */
    public static MinecraftServer currentServer() {
        return server;
    }

    /** Remember the running server so Rust-initiated actions can reach it. */
    public static void setServer(MinecraftServer s) { server = s; }

    /** Whether the online player carries command tag `tag` (`/tag`; saved with the player's own data). */
    public static boolean playerHasTag(String player, String tag) {
        ServerPlayer p = playerByName(player);
        return p != null && p.getTags().contains(tag);
    }

    /** Adds command tag `tag` to the online player; false when offline or already tagged. */
    public static boolean addPlayerTag(String player, String tag) {
        ServerPlayer p = playerByName(player);
        return p != null && p.addTag(tag);
    }

    /** Send a raw-byte packet to everyone who sees entity {@code uuid}, and to it if a player. */
    public static boolean sendToTracking(String uuid, String channel, byte[] data) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        YogPlatform.sendToTracking(e, channel, data);
        return true;
    }

    public static ServerPlayer playerByName(String name) {
        MinecraftServer s = server;
        return s == null ? null : s.getPlayerList().getPlayerByName(name);
    }

    /** The block container at (dimension, x, y, z) — a chest, a Yog inventory block — or null. */
    public static net.minecraft.world.Container containerAt(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        return w.getBlockEntity(new net.minecraft.core.BlockPos(x, y, z)) instanceof net.minecraft.world.Container c ? c : null;
    }

    // ── world / entity queries & actions ────────────────────────────────────

    /** Entity by uuid, or null for null / "" / unknown. */
    public static Entity optionalEntity(String uuid) {
        return uuid == null || uuid.isEmpty() ? null : entityByUuid(uuid);
    }

    /** Item stack from id + count + optional snbt tag; null if the id is unknown. */
    public static ItemStack buildStack(String itemId, int count, String snbt) {
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (id == null || count <= 0 || !BuiltInRegistries.ITEM.containsKey(id)) return null;
        ItemStack stack = new ItemStack(BuiltInRegistries.ITEM.get(id), count);
        if (snbt != null && !snbt.isEmpty()) {
            try {
                CompoundTag nbt = TagParser.parseTag(snbt);
                if (!nbt.isEmpty()) stack.setTag(nbt);
            } catch (CommandSyntaxException e) {
                YogLog.warn("item " + itemId + ": its data isn't SNBT (" + e.getMessage() + "): " + snbt);
            }
        }
        return stack;
    }

    /** Snbt of a stack's tag ("{}" if none). */
    public static String stackSnbt(ItemStack stack) {
        return stack.hasTag() ? stack.getTag().toString() : "{}";
    }

    /** {@code YogEntityFlags} bits of {@code e}. */
    public static int entityFlags(Entity e) {
        int flags = 0;
        if (e instanceof LivingEntity) flags |= YogEntityFlags.LIVING;
        if (e instanceof Player) flags |= YogEntityFlags.PLAYER;
        if (e instanceof net.minecraft.world.entity.animal.Animal
                || e instanceof net.minecraft.world.entity.animal.WaterAnimal) flags |= YogEntityFlags.ANIMAL;
        if (e instanceof net.minecraft.world.entity.monster.Enemy) flags |= YogEntityFlags.MONSTER;
        if (e instanceof ItemEntity) flags |= YogEntityFlags.ITEM;
        if (e instanceof net.minecraft.world.entity.Mob) flags |= YogEntityFlags.MOB;
        if (e instanceof net.minecraft.world.entity.decoration.ArmorStand) flags |= YogEntityFlags.ARMOR_STAND;
        if (e.isAlive()) flags |= YogEntityFlags.ALIVE;
        if (e.isSpectator()) flags |= YogEntityFlags.SPECTATOR;
        if (e instanceof net.minecraft.world.entity.boss.EnderDragonPart
                || YogPlatform.isPart(e)) flags |= YogEntityFlags.PART;
        if (e instanceof net.minecraft.world.entity.npc.Villager) flags |= YogEntityFlags.VILLAGER;
        if (e.isShiftKeyDown()) flags |= YogEntityFlags.SNEAKING;
        if (e.onGround()) flags |= YogEntityFlags.ON_GROUND;
        if (e.horizontalCollision) flags |= YogEntityFlags.HORIZONTAL_COLLISION;
        if (e instanceof net.minecraft.world.entity.LivingEntity gliding && gliding.isFallFlying()) flags |= YogEntityFlags.GLIDING;
        return flags;
    }

    public static Entity entityByUuid(String uuidStr) {
        MinecraftServer s = server;
        if (s == null) return null;
        for (ServerLevel w : s.getAllLevels()) {
            Entity e = w.getEntity(UUID.fromString(uuidStr));
            if (e != null) return e;
        }
        return null;
    }

    public static ServerLevel worldFor(String dimension) {
        MinecraftServer s = server;
        ResourceLocation id = ResourceLocation.tryParse(dimension);
        if (s == null || id == null) return null;
        return s.getLevel(ResourceKey.create(Registries.DIMENSION, id));
    }

    /** An entity's own hitbox (blocks), or vanilla's again (width &lt; 0). */
    public static boolean entitySetDimensions(String uuid, float width, float height, float eye) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        dev.yog.util.YogDimensions.set(false, e.getUUID(), width, height, eye);
        e.refreshDimensions();
        return true;
    }

    /** Power a Yog block declared as a redstone source gives off, by "dimension|x|y|z". */
    public static final java.util.Map<String, Integer> REDSTONE_OUTPUT = new java.util.concurrent.ConcurrentHashMap<>();

    public static String redstoneKey(String dimension, int x, int y, int z) {
        return dimension + "|" + x + "|" + y + "|" + z;
    }

    /** Load the embedded native runtime and initialise it. Idempotent. */
    public static synchronized void ensureLoaded() {
        if (loaded) return;
        YogNativeRuntime.loadEmbedded();
        // The host's actions, before the mods register (they may call them).
        YogHostServices.provideAll();
        nativeInit(YogNativeRuntime.modsDir(YogPlatform.gameDir()));
        // The mods' Yog Script, after the natives registered (it may call them).
        dev.yog.script.YsMods.load(YogPlatform.gameDir().resolve(YogNativeRuntime.MODS_DIR), net.minecraft.SharedConstants.getCurrentVersion().getName());
        loaded = true;
    }

    // ── native entry points ─────────────────────────────────────────────
    // (same as Fabric — JNI stubs implemented in Rust)

    public static native void nativeInit(String modsDir);
    /**
     * Ask the mod-registered generator type {@code generatorTypeId} to fill
     * chunk ({@code chunkX}, {@code chunkZ}). Blocks until the native
     * callback returns; every block it places arrives via a nested call
     * back into {@link YogCallbackChunkGenerator#setBlockInGeneratingChunk}
     * on this same thread (see {@code yog-dimensions}' {@code ChunkWriter}).
     */
    public static native void nativeGenerateChunk(
            String generatorTypeId, int chunkX, int chunkZ, int minY, int height);
    public static native void nativeUIShow(String uiId, String parentId, boolean modal, boolean pauseGame, int screenW, int screenH);
    public static native void nativeUIHide(String uiId);
    public static native boolean nativeUIClick(String uiId, float mx, float my, int button);

    /** Mouse wheel over a Yog UI screen; dy is the vertical scroll amount. */
    public static native void nativeUIScroll(String uiId, float dy);

    /** Mouse dragged over a Yog UI screen (any button held). */
    public static native void nativeUIDrag(String uiId, float mx, float my);
    /** The cursor moved over a Yog UI screen (`move:X:Y`). */
    public static native void nativeUIMove(String uiId, float mx, float my);

    /** Mouse button released over a Yog UI screen. */
    public static native void nativeUIRelease(String uiId, float mx, float my);
    /** Forward a key event (action: 0=release, 1=press, 2=repeat); true when the mod used it. */
    public static native boolean nativeUIKey(String uiId, int keyCode, int scanCode, int modifiers, int action);
    /** Forward a typed character (one UTF-16 unit); true when the mod used it. */
    public static native boolean nativeUIChar(String uiId, char chr, int modifiers);
    public static native void nativeUIRender(String uiId, int screenW, int screenH);
    public static native boolean nativeIsUIActive(String uiId);

    /** Everything registered — items, blocks, inventories — as one Yog Data blob ({@code YogContent}). */
    public static native byte[] nativeContent();

    public static native String nativePacketChannels();
    public static native String nativeClientPacketChannels();
    public static native void nativeGlInit();
    public static native void nativeOnHudRender(float deltaTick, int screenW, int screenH, float scaleFactor, float playerX, float playerY, float playerZ);
    public static native void nativeOnWorldRender(float deltaTick, int screenW, int screenH, float scaleFactor, float[] viewProj, float camX, float camY, float camZ, float playerX, float playerY, float playerZ);
    /** Resolve an OpenGL function pointer by name — used by yog-runtime via JNI. */
    public static long glProcAddress(String name) {
        return GLFW.glfwGetProcAddress(name);
    }

    // ── Inventory bridge (yog-ui ↔ YogInventoryMenu) ──────────────────────────

    public static YogInventoryMenu activeInventoryMenu;

    // ── Pictures in item tooltips (ABI 2.2) ─────────────────────────────────

    /** Entity data: everything about an entity, for a player who starts seeing it; null for nothing. */
    public static native byte[] nativeEntityDataFor(String uuid);
    /** Entity data: what the entity keeps when saved; null for nothing. */
    public static native byte[] nativeEntityDataSave(String uuid);
    /** Entity data: what it kept, back as it loads. */
    public static native void nativeEntityDataLoad(String uuid, byte[] saved);
    /** Entity data: the entity left the server's memory. */
    public static native void nativeEntityDataForget(String uuid);
    /** Sets ({@code value}) or removes (null) the host's own value about an entity, sent like a mod's, not saved. */
    public static native void nativeEntityDataSet(String uuid, String key, byte[] value);
    /** Entity data: the client left its world. */
    public static native void nativeEntityDataClientReset();

    /** What machine {@code defId} at a block makes of {@code items} ({@code \n}-joined by slot): {@code recipe\nresult\ncount}, or null. */
    public static native String nativeMachineResult(String defId, String dimension, int x, int y, int z, String items);

    /** A player took machine {@code defId}'s result of {@code recipe}: the mods take their share. */
    public static native boolean nativeMachineCraft(String defId, String recipe, String dimension, int x, int y, int z, String items, String player);

    /** Draws that picture in {@code x, y, w, h} (GUI pixels). */
    public static native void nativeTooltipDraw(String item, String snbt, int x, int y, int w, int h, int screenW, int screenH);

    /** A stack's custom data as SNBT, what tooltip pictures are told. */
    public static String tooltipSnbt(net.minecraft.world.item.ItemStack stack) {
        return stackSnbt(stack);
    }

    // ── Other mods (ABI 2.3) ────────────────────────────────────────────────

    /** Whether Java mod {@code id} is loaded (Yog mods are the runtime's to know). */
    public static boolean isModLoaded(String id) {
        try {
            return YogPlatform.isModLoaded(id);
        } catch (Throwable t) {
            return false;
        }
    }

    static {
        // How java_call finds an entity by uuid, and an entity's uuid.
        dev.yog.data.YogJava.entityByUuid = NativeBridge::entityByUuid;
        dev.yog.data.YogJava.uuidOf = o -> o instanceof Entity e ? e.getStringUUID() : null;
        // Which vanilla items this version has, for content named by another version's ids.
        dev.yog.data.YogVersions.itemExists = id -> { var r = net.minecraft.resources.ResourceLocation.tryParse(id); return r != null && net.minecraft.core.registries.BuiltInRegistries.ITEM.containsKey(r); };
    }

    /** A hopper or dropper offers {@code stack} to the Yog block at {@code pos} ({@code block.accepts_items}). */
    public static boolean itemInsert(net.minecraft.world.level.Level level, net.minecraft.core.BlockPos pos,
                                     ItemStack stack, boolean simulate) {
        String block = BuiltInRegistries.BLOCK.getKey(level.getBlockState(pos).getBlock()).toString();
        String item = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        return new dev.yog.bus.YogServices.Block.Insert(block, new dev.yog.bus.YogTypes.BlockPos(pos.getX(), pos.getY(), pos.getZ()),
                level.dimension().location().toString(), item, stack.getCount(), stackSnbt(stack), simulate).call();
    }
}
