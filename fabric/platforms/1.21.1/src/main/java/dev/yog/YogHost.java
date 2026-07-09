package dev.yog;

import com.mojang.brigadier.Command;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.context.CommandContext;
import net.minecraft.command.argument.BlockPosArgumentType;
import net.minecraft.command.argument.EntityArgumentType;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.itemgroup.v1.FabricItemGroup;
import net.fabricmc.fabric.api.registry.FuelRegistry;
import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.component.type.FoodComponent;
import net.minecraft.item.Item;
import net.minecraft.item.ItemConvertible;
import net.minecraft.item.ItemGroup;
import net.minecraft.registry.Registry;
import net.minecraft.sound.BlockSoundGroup;
import net.minecraft.util.Identifier;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerEntityEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents;
import net.fabricmc.fabric.api.entity.event.v1.ServerPlayerEvents;
import net.fabricmc.fabric.api.event.player.AttackEntityCallback;
import net.fabricmc.fabric.api.event.player.UseEntityCallback;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseBlockCallback;
import net.fabricmc.fabric.api.event.player.UseItemCallback;
import net.fabricmc.fabric.api.message.v1.ServerMessageEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.item.BlockItem;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.command.ServerCommandSource;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.text.Text;
import net.minecraft.util.ActionResult;
import net.minecraft.util.TypedActionResult;
import net.minecraft.block.entity.BlockEntityType;
import net.minecraft.block.entity.BlockEntity;
import net.fabricmc.fabric.api.screenhandler.v1.ExtendedScreenHandlerType;
import net.minecraft.network.codec.PacketCodecs;

/**
 * Fabric entry point. Boots the native Yog runtime and forwards server events
 * to it via {@link NativeBridge}. "The Gate and the Key."
 *
 * <p>We use Fabric API events rather than raw Mixins here: they are more stable
 * across mapping/version changes. Mixins return later for deeper hooks (e.g.
 * client rendering) that Fabric API does not cover.
 */
public class YogHost implements ModInitializer {
    /** UUID → [name, retriesLeft] for players waiting to appear in the server list. */
    private static final java.util.concurrent.ConcurrentHashMap<String, String[]> PENDING_JOINS =
            new java.util.concurrent.ConcurrentHashMap<>();

    // ── yog-inventory (see rust/crates/yog-inventory/DESIGN.md) ────────────────

    /** Parsed `InventoryDef`s, keyed by id — shared by the block entity/menu/screen. */
    public static final Map<String, InventoryDefRt> INVENTORY_DEFS = new HashMap<>();
    /** One generic block entity type valid for every block with an `inventory_id`. */
    public static BlockEntityType<YogInventoryBlockEntity> INVENTORY_BLOCK_ENTITY_TYPE;
    /** One generic menu type for every `InventoryDef` — the def id is synced as the
     *  screen-opening data so the client knows which layout/slot-count to build. */
    public static ExtendedScreenHandlerType<YogInventoryMenu, String> INVENTORY_SCREEN_HANDLER_TYPE;

    /** Runtime-side mirror of `yog_inventory::InventoryDef` — parsed from `nativeInventoryDefs()`. */
    public static final class InventoryDefRt {
        public final String id;
        public final int slotCount;
        public final List<float[]> layout;
        public final boolean includePlayerInventory;
        public final float playerInvX, playerInvY;
        public final String backgroundTexture;
        public final String title;

        InventoryDefRt(String id, int slotCount, List<float[]> layout, boolean includePlayerInventory,
                       float playerInvX, float playerInvY, String backgroundTexture, String title) {
            this.id = id;
            this.slotCount = slotCount;
            this.layout = layout;
            this.includePlayerInventory = includePlayerInventory;
            this.playerInvX = playerInvX;
            this.playerInvY = playerInvY;
            this.backgroundTexture = backgroundTexture;
            this.title = title;
        }
    }

    private static void parseInventoryDefs() {
        String raw = NativeBridge.nativeInventoryDefs();
        if (raw == null) return;
        for (String line : raw.split("\n")) {
            if (line.isBlank()) continue;
            String id = line.split("\t", 2)[0];
            Map<String, String> p = YogProps.parse(line);
            int slotCount = YogProps.parseInt(p, "slot_count", 0);
            List<float[]> layout = new ArrayList<>();
            String layoutRaw = p.getOrDefault("layout", "");
            if (!layoutRaw.isEmpty()) {
                for (String pair : layoutRaw.split(",")) {
                    String[] xy = pair.split(":", 2);
                    if (xy.length == 2) {
                        try {
                            layout.add(new float[]{Float.parseFloat(xy[0]), Float.parseFloat(xy[1])});
                        } catch (NumberFormatException ignored) { }
                    }
                }
            }
            boolean includePlayerInv = !"0".equals(p.get("include_player_inventory"));
            float px = 8f, py = 84f;
            String playerInv = p.getOrDefault("player_inv", "");
            String[] pxy = playerInv.split(":", 2);
            if (pxy.length == 2) {
                try { px = Float.parseFloat(pxy[0]); py = Float.parseFloat(pxy[1]); }
                catch (NumberFormatException ignored) { }
            }
            INVENTORY_DEFS.put(id, new InventoryDefRt(id, slotCount, layout, includePlayerInv, px, py,
                    p.getOrDefault("background_texture", ""), p.getOrDefault("title", "")));
        }
    }

    @Override
    public void onInitialize() {
        NativeBridge.ensureLoaded();
        System.out.println("[yog] Fabric host initialised.");

        // Register mod-declared content now, before the registries freeze.
        registerContent();

        // Server-side packet receivers (client -> server), raw bytes.
        String channels = NativeBridge.nativePacketChannels();
        if (channels != null) {
            for (String channel : channels.split("\n")) {
                if (channel.isBlank()) {
                    continue;
                }
                Identifier id = Identifier.tryParse(channel);
                if (id == null) {
                    continue;
                }
                YogPayload.register(id);
                ServerPlayNetworking.registerGlobalReceiver(YogPayload.idFor(id), (payload, context) -> {
                    byte[] data = payload.data();
                    var player = context.player();
                    player.getServer().execute(() ->
                            NativeBridge.nativeOnPacket(channel, player.getName().getString(), data));
                });
            }
        }

        // Channels a mod only ever *sends* to a client (e.g. `send_to_player`
        // from an `on_use_block` handler, to trigger a client-side `open_ui`)
        // are declared via `on_client_packet` — the server itself never
        // receives them, but Fabric's 1.20.5+ typed networking still requires
        // the S2C codec to be registered here before `ServerPlayNetworking.send`
        // will accept them, or the send throws and silently no-ops the whole
        // interaction. Register those too (registration is idempotent).
        String clientChannels = NativeBridge.nativeClientPacketChannels();
        if (clientChannels != null) {
            for (String channel : clientChannels.split("\n")) {
                if (channel.isBlank()) continue;
                Identifier id = Identifier.tryParse(channel);
                if (id == null) continue;
                YogPayload.register(id);
            }
        }

        // Block break — pre (cancellable) then after (observe).
        PlayerBlockBreakEvents.BEFORE.register((world, player, pos, state, blockEntity) -> {
            String blockId = Registries.BLOCK.getId(state.getBlock()).toString();
            return NativeBridge.nativeOnBlockBreakPre(
                    player.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
        });
        PlayerBlockBreakEvents.AFTER.register((world, player, pos, state, blockEntity) -> {
            String blockId = Registries.BLOCK.getId(state.getBlock()).toString();
            NativeBridge.nativeOnBlockBreak(
                    player.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
        });

        // Chat — pre (cancellable).
        ServerMessageEvents.ALLOW_CHAT_MESSAGE.register((message, sender, params) ->
                NativeBridge.nativeOnChatPre(
                        sender.getName().getString(), message.getContent().getString()));

        // Chat — after (observe).
        ServerMessageEvents.CHAT_MESSAGE.register((message, sender, params) ->
                NativeBridge.nativeOnChat(
                        sender.getName().getString(), message.getContent().getString()));

        // Player join / leave.
        // We don't call nativeOnPlayerJoin immediately because ServerPlayConnectionEvents.JOIN
        // fires before the player is fully in the server's player list. We queue the UUID and
        // check each server tick (via END_SERVER_TICK) until the player appears.
        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {
            String pUuid = handler.player.getUuidAsString();
            String pName = handler.player.getName().getString();
            PENDING_JOINS.put(pUuid, new String[]{pName, "40"});
        });

        ServerPlayConnectionEvents.DISCONNECT.register((handler, server) -> {
            String pUuid = handler.player.getUuidAsString();
            PENDING_JOINS.remove(pUuid); // cancel pending join if player disconnects before we process it
            NativeBridge.nativeOnPlayerLeave(
                    handler.player.getName().getString(), pUuid);
        });

        // Item use (right-click), server side only.
        UseItemCallback.EVENT.register((player, world, hand) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                ItemStack stack = sp.getStackInHand(hand);
                String itemId = Registries.ITEM.getId(stack.getItem()).toString();
                boolean allow = NativeBridge.nativeOnUseItemPre(
                        sp.getName().getString(), itemId, sp.isSneaking());
                if (!allow) return TypedActionResult.fail(stack);
                NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId, sp.isSneaking());
            }
            return TypedActionResult.pass(player.getStackInHand(hand));
        });

        // Block use (right-click on a block), server side only.
        UseBlockCallback.EVENT.register((player, world, hand, hitResult) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                net.minecraft.util.math.BlockPos pos = hitResult.getBlockPos();
                String blockId = Registries.BLOCK.getId(world.getBlockState(pos).getBlock()).toString();
                boolean allow = NativeBridge.nativeOnUseBlockPre(
                        sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
                if (!allow) return ActionResult.FAIL;
                NativeBridge.nativeOnUseBlock(
                        sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
            }
            return ActionResult.PASS;
        });

        // Block placement — Pre (cancellable). Fires when a player uses a BlockItem on a surface.
        UseBlockCallback.EVENT.register((player, world, hand, hitResult) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                ItemStack held = sp.getStackInHand(hand);
                if (held.getItem() instanceof BlockItem bi) {
                    net.minecraft.util.math.BlockPos placed =
                            hitResult.getBlockPos().offset(hitResult.getSide());
                    String blockId = Registries.BLOCK.getId(bi.getBlock()).toString();
                    if (!NativeBridge.nativeOnPlaceBlockPre(
                            sp.getName().getString(), blockId,
                            placed.getX(), placed.getY(), placed.getZ())) {
                        return ActionResult.FAIL;
                    }
                }
            }
            return ActionResult.PASS;
        });

        // Entity interact (right-click on entity) — Pre (cancellable) then Post.
        UseEntityCallback.EVENT.register((player, world, hand, entity, hitResult) -> {
            if (!world.isClient && player instanceof net.minecraft.server.network.ServerPlayerEntity sp) {
                String pName    = sp.getName().getString();
                String pUuid    = sp.getUuidAsString();
                String eType    = net.minecraft.registry.Registries.ENTITY_TYPE.getId(entity.getType()).toString();
                String eUuid    = entity.getUuidAsString();
                String handStr  = hand == net.minecraft.util.Hand.MAIN_HAND ? "main_hand" : "off_hand";
                boolean allow = NativeBridge.nativeOnEntityInteractPre(pName, pUuid, eType, eUuid, handStr);
                if (!allow) return ActionResult.FAIL;
                NativeBridge.nativeOnEntityInteract(pName, pUuid, eType, eUuid, handStr);
            }
            return ActionResult.PASS;
        });

        // Attack (left-click on an entity), server side only.
        AttackEntityCallback.EVENT.register((player, world, hand, entity, hitResult) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
                NativeBridge.nativeOnAttackEntity(
                        sp.getName().getString(), type, entity.getUuidAsString());
            }
            return ActionResult.PASS;
        });

        // Living-entity damage — pre (cancellable) then observe.
        ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> {
            String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
            boolean allow = NativeBridge.nativeOnEntityDamagePre(
                    type, entity.getUuidAsString(), amount, source.getName());
            if (allow) {
                NativeBridge.nativeOnEntityDamage(
                        type, entity.getUuidAsString(), amount, source.getName());
            }
            return allow;
        });

        // Entity spawn — fire when an entity is loaded into a server world.
        ServerEntityEvents.ENTITY_LOAD.register((entity, world) -> {
            String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
            String uuid = entity.getUuidAsString();
            String dim  = world.getRegistryKey().getValue().toString();
            // Pre (cancellable) — discard entity if any handler returns false.
            if (!NativeBridge.nativeOnEntitySpawnPre(type, uuid, dim)) {
                entity.discard();
                return;
            }
            // Observe-only handlers.
            NativeBridge.nativeOnEntitySpawn(type, uuid, dim);
        });

        // Living-entity death (server side — non-player entity post).
        ServerLivingEntityEvents.AFTER_DEATH.register((entity, source) -> {
            if (entity instanceof ServerPlayerEntity) return; // handled separately below
            String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
            NativeBridge.nativeOnEntityDeath(
                    type, entity.getUuidAsString(), source.getName());
        });

        // Player death — pre (cancellable) and post.
        ServerLivingEntityEvents.ALLOW_DEATH.register((entity, source, amount) -> {
            if (!(entity instanceof ServerPlayerEntity sp)) return true;
            boolean allow = NativeBridge.nativeOnPlayerDeathPre(
                    sp.getName().getString(), sp.getUuidAsString(), source.getName());
            if (!allow) return false;
            NativeBridge.nativeOnPlayerDeath(
                    sp.getName().getString(), sp.getUuidAsString(), source.getName());
            return true;
        });

        // Player respawn.
        ServerPlayerEvents.AFTER_RESPAWN.register((oldPlayer, newPlayer, alive) -> {
            if (alive) return; // dimension-change respawn, not death respawn
            NativeBridge.nativeOnPlayerRespawn(
                    newPlayer.getName().getString(), newPlayer.getUuidAsString(), false);
        });

        // End-of-tick (20×/second).
        ServerTickEvents.END_SERVER_TICK.register(server -> {
            // Resolve pending player joins: fire nativeOnPlayerJoin once the player is in the list.
            if (!PENDING_JOINS.isEmpty()) {
                java.util.List<String> toRemove = new java.util.ArrayList<>();
                PENDING_JOINS.forEach((uuid, entry) -> {
                    String name = entry[0];
                    int retries = Integer.parseInt(entry[1]);
                    java.util.UUID parsed = java.util.UUID.fromString(uuid);
                    ServerPlayerEntity found = server.getPlayerManager().getPlayer(parsed);
                    if (found != null) {
                        NativeBridge.nativeOnPlayerJoin(found.getName().getString(), uuid);
                        toRemove.add(uuid);
                    } else if (retries <= 0) {
                        System.out.println("[yog] player join: " + name + " never appeared after retries");
                        toRemove.add(uuid);
                    } else {
                        entry[1] = String.valueOf(retries - 1);
                    }
                });
                toRemove.forEach(PENDING_JOINS::remove);
            }
            NativeBridge.nativeOnTick();
        });

        // Server lifecycle. Capture the server first so Rust can act on it
        // (e.g. NativeBridge.broadcast).
        ServerLifecycleEvents.SERVER_STARTED.register(server -> {
            NativeBridge.setServer(server);
            String worldDir = server.getSavePath(net.minecraft.util.WorldSavePath.ROOT)
                    .toAbsolutePath().toString();
            NativeBridge.nativeOnServerStarted(worldDir);
        });
        ServerLifecycleEvents.SERVER_STOPPING.register(server -> NativeBridge.nativeOnServerStopping());

        // Commands: register each mod-declared command with Brigadier and route to Rust.
        CommandRegistrationCallback.EVENT.register((dispatcher, registryAccess, environment) -> {
            // Typed commands: build proper Brigadier argument chains. A single
            // command name can have MULTIPLE distinct schemas (subcommands with
            // different arg counts) — each is registered separately; Brigadier
            // merges repeated `dispatcher.register()` calls that share a root
            // literal name into one command-tree node, so this produces one
            // "vlsi" node with a sibling branch per distinct arg shape, instead
            // of collapsing them all down to whichever schema was seen last.
            java.util.Set<String> typedNames = new java.util.HashSet<>();
            String schemaLines = NativeBridge.nativeTypedCommandSchemas();
            if (schemaLines != null) {
                for (String line : schemaLines.split("\n")) {
                    if (line.isBlank()) continue;
                    int tab = line.indexOf('\t');
                    if (tab <= 0) continue;
                    String name   = line.substring(0, tab);
                    String schema = line.substring(tab + 1);
                    typedNames.add(name);
                    dispatcher.register(buildTypedCommand(name, schema.split("\\s+")));
                }
            }

            // Plain commands: greedy-string arg (or no args).
            String names = NativeBridge.nativeCommandNames();
            if (names == null || names.isBlank()) return;
            for (String name : names.split("\n")) {
                if (name.isBlank()) continue;
                dispatcher.register(CommandManager.literal(name)
                        .executes(ctx -> runCommand(name, "", ctx))
                        .then(CommandManager.argument("args", StringArgumentType.greedyString())
                                .executes(ctx -> runCommand(
                                        name, StringArgumentType.getString(ctx, "args"), ctx))));
            }
        });
    }

    /**
     * Register custom items/blocks declared by Rust mods, apply their name and
     * tooltip, and collect them into per-namespace creative tabs.
     */
    /** Parse `id\tkey=value\t...` into a map. First element is the id. */
    
    private static void registerContent() {
        parseInventoryDefs();

        // Group items and blocks by namespace for per-mod creative tabs.
        Map<String, List<ItemConvertible>> tabGroups = new LinkedHashMap<>();
        List<Block> inventoryBlocks = new ArrayList<>();

        // Mods register a block's item form as `register_item(same_id, name, tooltip)`
        // alongside `register_block(same_id)` — BlockDef itself carries neither. Collect
        // those ids up front so the item loop below hands their name/tooltip to the
        // block's YogBlockItem instead of ALSO registering a second, non-block Item
        // under the same Identifier (that used to silently collide in Registries.ITEM
        // and show up as a duplicate, unplaceable ghost entry in the creative tab).
        Set<String> blockIds = new HashSet<>();
        String blocksRaw = NativeBridge.nativeBlockDefs();
        if (blocksRaw != null) {
            for (String line : blocksRaw.split("\n")) {
                if (line.isBlank()) continue;
                blockIds.add(line.split("\t", 2)[0]);
            }
        }
        Map<String, String> blockItemNames = new HashMap<>();
        Map<String, String> blockItemTooltips = new HashMap<>();

        String items = NativeBridge.nativeItemDefs();
        if (items != null) {
            for (String line : items.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = YogProps.parse(line);

                if (blockIds.contains(id)) {
                    blockItemNames.put(id, p.getOrDefault("name", ""));
                    blockItemTooltips.put(id, p.getOrDefault("tooltip", ""));
                    continue;
                }

                Item.Settings settings = new Item.Settings();

                int maxDamage = YogProps.parseInt(p, "max_damage", 0);
                if (maxDamage > 0) {
                    settings = settings.maxDamage(maxDamage);
                } else {
                    settings = settings.maxCount(YogProps.parseInt(p, "max_stack", 64));
                }

                if ("1".equals(p.get("fire_resistant"))) settings = settings.fireproof();

                if (p.containsKey("food")) {
                    String[] fp = p.get("food").split(":", 3);
                    if (fp.length >= 2) {
                        FoodComponent.Builder fb = new FoodComponent.Builder()
                                .nutrition(Integer.parseInt(fp[0]))
                                .saturationModifier(Float.parseFloat(fp[1]));
                    if ("1".equals(fp.length > 2 ? fp[2] : "0")) fb = fb.alwaysEdible();
                    FoodComponent food = fb.build();
                        settings = settings.food(food);
                    }
                }

                // Check if this item has an associated book
                String bookJson = NativeBridge.nativeBookJson(id);
                Item item;
                if (bookJson != null && !bookJson.equals("null")) {
                    item = new YogBookItem(settings,
                            p.getOrDefault("name", ""), p.getOrDefault("tooltip", ""), id);
                } else {
                    item = new YogItem(settings,
                            p.getOrDefault("name", ""), p.getOrDefault("tooltip", ""));
                }
                Registry.register(Registries.ITEM, ident, item);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(item);

                int fuelTicks = YogProps.parseInt(p, "fuel_ticks", 0);
                if (fuelTicks > 0) FuelRegistry.INSTANCE.add(item, fuelTicks);
            }
        }

        String blocks = blocksRaw;
        if (blocks != null) {
            for (String line : blocks.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = YogProps.parse(line);
                float hardness   = YogProps.parseFloat(p, "hardness",   1.5f);
                float resistance = YogProps.parseFloat(p, "resistance",  6.0f);

                AbstractBlock.Settings settings = AbstractBlock.Settings.create()
                        .strength(hardness, resistance);

                if (p.containsKey("light")) {
                    int lv = YogProps.parseInt(p, "light", 0);
                    settings = settings.luminance(state -> lv);
                }
                if (p.containsKey("sound")) {
                    settings = settings.sounds(blockSoundGroup(p.get("sound")));
                }
                if ("1".equals(p.get("requires_tool"))) settings = settings.requiresTool();
                if ("1".equals(p.get("no_collision")))  settings = settings.noCollision();
                if (p.containsKey("slipperiness")) {
                    settings = settings.slipperiness(YogProps.parseFloat(p, "slipperiness", 0.6f));
                }

                Block block;
                if (p.containsKey("inventory_id")) {
                    InventoryDefRt def = INVENTORY_DEFS.get(p.get("inventory_id"));
                    int slotCount = def != null ? def.slotCount : 0;
                    block = new YogInventoryBlock(settings, p.get("inventory_id"), slotCount);
                    inventoryBlocks.add(block);
                } else if ("1".equals(p.get("connects"))) {
                    double[] core = {6, 0, 6, 10, 16, 10};
                    if (p.containsKey("shape")) {
                        String[] sp = p.get("shape").split(":", 6);
                        for (int i = 0; i < 6; i++) core[i] = Double.parseDouble(sp[i]);
                    }
                    block = new YogConnectingBlock(settings, core[0], core[1], core[2], core[3], core[4], core[5]);
                } else if (p.containsKey("shape")) {
                    String[] sp = p.get("shape").split(":", 6);
                    block = new YogShapedBlock(settings,
                            Double.parseDouble(sp[0]), Double.parseDouble(sp[1]),
                            Double.parseDouble(sp[2]), Double.parseDouble(sp[3]),
                            Double.parseDouble(sp[4]), Double.parseDouble(sp[5]));
                } else {
                    block = new Block(settings);
                }

                Registry.register(Registries.BLOCK, ident, block);
                if (p.containsKey("connect_groups")) {
                    YogConnectingLogic.registerGroups(block, p.get("connect_groups").split(","));
                }
                Item blockItem = new YogBlockItem(block, new Item.Settings(),
                        blockItemNames.getOrDefault(id, ""),
                        blockItemTooltips.getOrDefault(id, ""));
                Registry.register(Registries.ITEM, ident, blockItem);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(blockItem);
            }
        }

        // One generic block entity type valid for every inventory-backed block,
        // and one generic menu type for every InventoryDef (see yog-inventory's DESIGN.md).
        if (!inventoryBlocks.isEmpty()) {
            INVENTORY_BLOCK_ENTITY_TYPE = Registry.register(Registries.BLOCK_ENTITY_TYPE,
                    Identifier.of("yog", "inventory"),
                    BlockEntityType.Builder.<YogInventoryBlockEntity>create(
                            (pos, state) -> {
                                YogInventoryBlock ib = (YogInventoryBlock) state.getBlock();
                                return new YogInventoryBlockEntity(pos, state, ib.defId(), ib.slotCount());
                            },
                            inventoryBlocks.toArray(new Block[0])
                    ).build(null));
        }
        INVENTORY_SCREEN_HANDLER_TYPE = Registry.register(Registries.SCREEN_HANDLER,
                Identifier.of("yog", "inventory"),
                new ExtendedScreenHandlerType<>(YogInventoryMenu::createClient, PacketCodecs.STRING));

        // Create one creative tab per namespace, using the first item from each as icon.
        for (Map.Entry<String, List<ItemConvertible>> entry : tabGroups.entrySet()) {
            String ns = entry.getKey();
            List<ItemConvertible> entries = entry.getValue();
            if (entries.isEmpty()) continue;

            ItemConvertible icon = entries.get(0);
            ItemGroup group = FabricItemGroup.builder()
                    .icon(() -> new ItemStack(icon))
                    .displayName(Text.literal(ns))
                    .entries((displayContext, tabEntries) -> entries.forEach(tabEntries::add))
                    .build();
            Registry.register(Registries.ITEM_GROUP, Identifier.of(ns, ns), group);
        }
    }

    
    
    private static BlockSoundGroup blockSoundGroup(String name) {
        return switch (name) {
            case "wood"         -> BlockSoundGroup.WOOD;
            case "grass"        -> BlockSoundGroup.GRASS;
            case "gravel"       -> BlockSoundGroup.GRAVEL;
            case "sand"         -> BlockSoundGroup.SAND;
            case "snow"         -> BlockSoundGroup.SNOW;
            case "metal"        -> BlockSoundGroup.METAL;
            case "glass"        -> BlockSoundGroup.GLASS;
            case "wool"         -> BlockSoundGroup.WOOL;
            case "nether_brick" -> BlockSoundGroup.NETHER_BRICKS;
            default             -> BlockSoundGroup.STONE;
        };
    }

    /**
     * Build a Brigadier literal node with typed argument chain from a schema like
     * {@code ["int", "player", "blockpos"]}.  Each arg is named {@code arg_N}.
     * All resolved args are serialised tab-separated and forwarded to Rust.
     */
    private static LiteralArgumentBuilder<ServerCommandSource>
            buildTypedCommand(String name, String[] schema) {
        var root = CommandManager.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, "", ctx));
            return root;
        }
        // Build argument chain from last to first, wrapping inner with outer.
        ArgumentBuilder<ServerCommandSource, ?> chain = buildLeaf(name, schema, schema.length - 1);
        for (int i = schema.length - 2; i >= 0; i--) {
            chain = buildArgNode(schema[i], "arg_" + i).then(chain);
            // Also allow executing with fewer args (partial match not standard; just attach executes at leaf).
        }
        root.then(chain);
        return root;
    }

    private static RequiredArgumentBuilder<ServerCommandSource, ?> buildArgNode(String type, String argName) {
        return switch (type) {
            case "int"      -> CommandManager.argument(argName, IntegerArgumentType.integer());
            case "float"    -> CommandManager.argument(argName, FloatArgumentType.floatArg());
            case "word"     -> CommandManager.argument(argName, StringArgumentType.word());
            case "string"   -> CommandManager.argument(argName, StringArgumentType.greedyString());
            case "player"   -> CommandManager.argument(argName, EntityArgumentType.player());
            case "blockpos" -> CommandManager.argument(argName, BlockPosArgumentType.blockPos());
            default         -> CommandManager.argument(argName, StringArgumentType.word());
        };
    }

    private static ArgumentBuilder<ServerCommandSource, ?> buildLeaf(String cmdName, String[] schema, int idx) {
        String type    = schema[idx];
        String argName = "arg_" + idx;
        return buildArgNode(type, argName).executes(ctx -> {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i <= idx; i++) {
                if (i > 0) sb.append('\t');
                sb.append(resolveArg(schema[i], "arg_" + i, ctx));
            }
            return runCommand(cmdName, sb.toString(), ctx);
        });
    }

    private static String resolveArg(String type, String argName, CommandContext<ServerCommandSource> ctx) {
        try {
            return switch (type) {
                case "int"      -> String.valueOf(IntegerArgumentType.getInteger(ctx, argName));
                case "float"    -> String.valueOf(FloatArgumentType.getFloat(ctx, argName));
                case "word", "string" -> StringArgumentType.getString(ctx, argName);
                case "player"   -> EntityArgumentType.getPlayer(ctx, argName).getName().getString();
                case "blockpos" -> {
                    net.minecraft.util.math.BlockPos pos = BlockPosArgumentType.getBlockPos(ctx, argName);
                    yield pos.getX() + "," + pos.getY() + "," + pos.getZ();
                }
                default -> StringArgumentType.getString(ctx, argName);
            };
        } catch (Exception e) {
            return "";
        }
    }

    private static int runCommand(String name, String args, CommandContext<ServerCommandSource> ctx) {
        ServerCommandSource src = ctx.getSource();
        net.minecraft.entity.Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getUuidAsString() : "";
        String reply = NativeBridge.nativeOnCommand(name, args, src.getName(), uuid);
        if (reply != null && !reply.isEmpty()) {
            src.sendFeedback(() -> Text.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }
}
