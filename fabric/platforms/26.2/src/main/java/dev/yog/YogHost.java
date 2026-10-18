package dev.yog;

import dev.yog.data.YogContent;
import dev.yog.block.YogConnectingLogic;
import dev.yog.util.YogInventoryDef;

import com.mojang.brigadier.Command;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.context.CommandContext;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.creativetab.v1.FabricCreativeModeTab;
import net.fabricmc.fabric.api.registry.FuelValueEvents;
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
import net.fabricmc.fabric.api.menu.v1.ExtendedMenuType;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.EntityArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.ItemLike;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;

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
    public static final Map<String, YogInventoryDef> INVENTORY_DEFS = new HashMap<>();
    /** One generic block entity type valid for every block with an `inventory_id`. */
    public static BlockEntityType<YogInventoryBlockEntity> INVENTORY_BLOCK_ENTITY_TYPE;
    /** One generic menu type for every `InventoryDef` — the def id is synced as the
     *  screen-opening data so the client knows which layout/slot-count to build. */
    public static ExtendedMenuType<YogInventoryMenu, String> INVENTORY_SCREEN_HANDLER_TYPE;
    /** Burn time in ticks of each item the mods made fuel; the game asks for it whenever it builds its fuel values. */
    private static final Map<Item, Integer> FUELS = new LinkedHashMap<>();

    private static void parseInventoryDefs() {
        INVENTORY_DEFS.putAll(YogContent.cached(NativeBridge::nativeContent).inventories);
        // Features the mods add to biomes (their ores): Fabric adds them in code,
        // Forge and NeoForge read the biome modifiers the build wrote.
        for (YogContent.BiomeFeature f : YogContent.cached(NativeBridge::nativeContent).biomeFeatures) {
            Identifier feature = Identifier.tryParse(f.feature()), biomes = Identifier.tryParse(f.biomes());
            if (feature == null || biomes == null) continue;
            net.minecraft.world.level.levelgen.GenerationStep.Decoration step;
            try {
                step = net.minecraft.world.level.levelgen.GenerationStep.Decoration.valueOf(f.step().toUpperCase(java.util.Locale.ROOT));
            } catch (IllegalArgumentException e) {
                YogLog.warn("" + f.feature() + ": no generation step " + f.step());
                continue;
            }
            net.fabricmc.fabric.api.biome.v1.BiomeModifications.addFeature(
                    net.fabricmc.fabric.api.biome.v1.BiomeSelectors.tag(net.minecraft.tags.TagKey.create(net.minecraft.core.registries.Registries.BIOME, biomes)),
                    step, net.minecraft.resources.ResourceKey.create(net.minecraft.core.registries.Registries.PLACED_FEATURE, feature));
        }
    }

    @Override
    public void onInitialize() {
        FuelValueEvents.BUILD.register((fuels, context) -> FUELS.forEach(fuels::add));
        NativeBridge.ensureLoaded();
        YogLog.info("Fabric host initialised.");

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
                    player.level().getServer().execute(() ->
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

        // Entity data reaches who sees an entity: all of it when they start
        // seeing it or come into a world; an entity leaving the server's memory
        // is forgotten (what it keeps was saved with it).
        net.fabricmc.fabric.api.networking.v1.EntityTrackingEvents.START_TRACKING.register(
                (entity, player) -> sendEntityData(player, entity));
        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> sendEntityData(handler.player, handler.player));
        net.fabricmc.fabric.api.entity.event.v1.ServerPlayerEvents.AFTER_RESPAWN.register(
                (oldPlayer, newPlayer, alive) -> sendEntityData(newPlayer, newPlayer));
        net.fabricmc.fabric.api.entity.event.v1.ServerEntityLevelChangeEvents.AFTER_PLAYER_CHANGE_LEVEL.register(
                (player, origin, destination) -> sendEntityData(player, player));
        ServerEntityEvents.ENTITY_UNLOAD.register((entity, world) -> {
            if (!(entity instanceof net.minecraft.world.entity.player.Player)) {
                NativeBridge.nativeEntityDataForget(entity.getStringUUID());
            }
        });

        // Block break — pre (cancellable) then after (observe).
        PlayerBlockBreakEvents.BEFORE.register((world, player, pos, state, blockEntity) -> {
            String blockId = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
            String dim = world.dimension().identifier().toString();
            return NativeBridge.nativeOnBlockBreakPre(
                    player.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ(), dim);
        });
        PlayerBlockBreakEvents.AFTER.register((world, player, pos, state, blockEntity) -> {
            String blockId = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
            String dim = world.dimension().identifier().toString();
            NativeBridge.nativeOnBlockBreak(
                    player.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ(), dim);

            // Phase 6: drop inventory contents so items survive block breaking.
            if (blockEntity instanceof YogInventoryBlockEntity inv && !inv.isEmpty()) {
                net.minecraft.world.Containers.dropContents(world, pos, inv);
                inv.clearContent();
            }
        });

        // Chat — pre (cancellable).
        ServerMessageEvents.ALLOW_CHAT_MESSAGE.register((message, sender, params) ->
                NativeBridge.nativeOnChatPre(
                        sender.getName().getString(), message.decoratedContent().getString(),
                        sender.level().dimension().identifier().toString()));

        // Chat — after (observe).
        ServerMessageEvents.CHAT_MESSAGE.register((message, sender, params) ->
                NativeBridge.nativeOnChat(
                        sender.getName().getString(), message.decoratedContent().getString(),
                        sender.level().dimension().identifier().toString()));

        // Player join / leave.
        // We don't call nativeOnPlayerJoin immediately because ServerPlayConnectionEvents.JOIN
        // fires before the player is fully in the server's player list. We queue the UUID and
        // check each server tick (via END_SERVER_TICK) until the player appears.
        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {
            String pUuid = handler.player.getStringUUID();
            String pName = handler.player.getName().getString();
            PENDING_JOINS.put(pUuid, new String[]{pName, "40"});
        });

        ServerPlayConnectionEvents.DISCONNECT.register((handler, server) -> {
            String pUuid = handler.player.getStringUUID();
            PENDING_JOINS.remove(pUuid); // cancel pending join if player disconnects before we process it
            NativeBridge.nativeOnPlayerLeave(
                    handler.player.getName().getString(), pUuid,
                    handler.player.level().dimension().identifier().toString());
        });

        // Item use (right-click), server side only.
        UseItemCallback.EVENT.register((player, world, hand) -> {
            if (!world.isClientSide() && player instanceof ServerPlayer sp) {
                ItemStack stack = sp.getItemInHand(hand);
                // As vanilla (and Forge's event): an item cooling down isn't used.
                if (sp.getCooldowns().isOnCooldown(stack)) {
                    return InteractionResult.PASS;
                }
                String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
                String dim = world.dimension().identifier().toString();
                boolean allow = NativeBridge.nativeOnUseItemPre(
                        sp.getName().getString(), itemId, sp.isShiftKeyDown(), dim);
                if (!allow) return InteractionResult.FAIL;
                NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId, sp.isShiftKeyDown(), dim);
            }
            return InteractionResult.PASS;
        });

        // Block use (right-click on a block), server side only.
        UseBlockCallback.EVENT.register((player, world, hand, hitResult) -> {
            if (!world.isClientSide() && player instanceof ServerPlayer sp) {
                // As vanilla: sneaking with something in hand uses the item (places a block), not the block.
                if (sp.isSecondaryUseActive() && (!sp.getMainHandItem().isEmpty() || !sp.getOffhandItem().isEmpty())) {
                    return InteractionResult.PASS;
                }
                net.minecraft.core.BlockPos pos = hitResult.getBlockPos();
                String blockId = BuiltInRegistries.BLOCK.getKey(world.getBlockState(pos).getBlock()).toString();
                String dim = world.dimension().identifier().toString();
                boolean allow = NativeBridge.nativeOnUseBlockPre(
                        sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ(), dim, sp.isShiftKeyDown());
                if (!allow) return InteractionResult.FAIL;
                NativeBridge.nativeOnUseBlock(
                        sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ(), dim, sp.isShiftKeyDown());
            }
            return InteractionResult.PASS;
        });

        // Block placement — Pre (cancellable). Fires when a player uses a BlockItem on a surface.
        UseBlockCallback.EVENT.register((player, world, hand, hitResult) -> {
            if (!world.isClientSide() && player instanceof ServerPlayer sp) {
                ItemStack held = sp.getItemInHand(hand);
                if (held.getItem() instanceof BlockItem bi) {
                    net.minecraft.core.BlockPos placed =
                            hitResult.getBlockPos().relative(hitResult.getDirection());
                    String blockId = BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
                    if (!NativeBridge.nativeOnPlaceBlockPre(
                            sp.getName().getString(), blockId,
                            placed.getX(), placed.getY(), placed.getZ(),
                            world.dimension().identifier().toString(), sp.isShiftKeyDown())) {
                        return InteractionResult.FAIL;
                    }
                }
            }
            return InteractionResult.PASS;
        });

        // Entity interact (right-click on entity) — Pre (cancellable) then Post.
        UseEntityCallback.EVENT.register((player, world, hand, entity, hitResult) -> {
            if (!world.isClientSide() && player instanceof net.minecraft.server.level.ServerPlayer sp) {
                String pName    = sp.getName().getString();
                String pUuid    = sp.getStringUUID();
                String eType    = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
                String eUuid    = entity.getStringUUID();
                String handStr  = hand == net.minecraft.world.InteractionHand.MAIN_HAND ? "main_hand" : "off_hand";
                String dim      = world.dimension().identifier().toString();
                boolean allow = NativeBridge.nativeOnEntityInteractPre(pName, pUuid, eType, eUuid, handStr, dim);
                if (!allow) return InteractionResult.FAIL;
                NativeBridge.nativeOnEntityInteract(pName, pUuid, eType, eUuid, handStr, dim);
            }
            return InteractionResult.PASS;
        });

        // Attack (left-click on an entity), server side only.
        AttackEntityCallback.EVENT.register((player, world, hand, entity, hitResult) -> {
            if (!world.isClientSide() && player instanceof ServerPlayer sp) {
                String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
                NativeBridge.nativeOnAttackEntity(
                        sp.getName().getString(), type, entity.getStringUUID(),
                        world.dimension().identifier().toString());
            }
            return InteractionResult.PASS;
        });

        // Living-entity damage — pre (cancellable) then observe.
        ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> {
            String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
            String dim = entity.level().dimension().identifier().toString();
            boolean allow = NativeBridge.nativeOnEntityDamagePre(
                    type, entity.getStringUUID(), amount, source.getMsgId(), dim);
            if (allow) {
                NativeBridge.nativeOnEntityDamage(
                        type, entity.getStringUUID(), amount, source.getMsgId(), dim);
            }
            return allow;
        });

        // Entity spawn — fire when an entity is loaded into a server world.
        ServerEntityEvents.ENTITY_LOAD.register((entity, world) -> {
            String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
            String uuid = entity.getStringUUID();
            String dim  = world.dimension().identifier().toString();
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
            if (entity instanceof ServerPlayer) return; // handled separately below
            String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
            NativeBridge.nativeOnEntityDeath(
                    type, entity.getStringUUID(), source.getMsgId(),
                    entity.level().dimension().identifier().toString());
        });

        // Player death — pre (cancellable) and post.
        ServerLivingEntityEvents.ALLOW_DEATH.register((entity, source, amount) -> {
            if (!(entity instanceof ServerPlayer sp)) return true;
            String dim = sp.level().dimension().identifier().toString();
            boolean allow = NativeBridge.nativeOnPlayerDeathPre(
                    sp.getName().getString(), sp.getStringUUID(), source.getMsgId(), dim);
            if (!allow) return false;
            NativeBridge.nativeOnPlayerDeath(
                    sp.getName().getString(), sp.getStringUUID(), source.getMsgId(), dim);
            return true;
        });

        // Player respawn.
        ServerPlayerEvents.AFTER_RESPAWN.register((oldPlayer, newPlayer, alive) -> {
            if (alive) return; // dimension-change respawn, not death respawn
            NativeBridge.nativeOnPlayerRespawn(
                    newPlayer.getName().getString(), newPlayer.getStringUUID(), false,
                    newPlayer.level().dimension().identifier().toString());
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
                    ServerPlayer found = server.getPlayerList().getPlayer(parsed);
                    if (found != null) {
                        NativeBridge.nativeOnPlayerJoin(found.getName().getString(), uuid,
                                found.level().dimension().identifier().toString());
                        toRemove.add(uuid);
                    } else if (retries <= 0) {
                        YogLog.warn("player join: " + name + " never appeared after retries");
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
            String worldDir = server.getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT)
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
            // root node with a sibling branch per distinct arg shape, instead
            // of collapsing them all down to whichever schema was seen last.
            java.util.Set<String> typedNames = new java.util.HashSet<>();
            String schemaLines = NativeBridge.nativeTypedCommandSchemas();
            if (schemaLines != null) {
                for (String line : typedSchemaOrder(schemaLines)) {
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
                dispatcher.register(Commands.literal(name)
                        .executes(ctx -> runCommand(name, "", ctx))
                        .then(Commands.argument("args", StringArgumentType.greedyString())
                                .executes(ctx -> runCommand(
                                        name, StringArgumentType.getString(ctx, "args").trim().replaceAll("\\s+", "\t"), ctx))));
            }
        });
    }

    /**
     * Register custom items/blocks declared by Rust mods, apply their name and
     * tooltip, and collect them into per-namespace creative tabs.
     */
    /** Parse `id\tkey=value\t...` into a map. First element is the id. */
    
    private static void registerContent() {
        YogCallbackChunkGenerator.registerCodec();
        // Mods' entity types first: spawn eggs (items) name them.
        YogEntityTypes.register(YogContent.cached(NativeBridge::nativeContent).entities);
        YogEffects.register(YogContent.cached(NativeBridge::nativeContent));

        parseInventoryDefs();

        // Group items and blocks by namespace for per-mod creative tabs.
        Map<String, List<ItemLike>> tabGroups = new LinkedHashMap<>();
        List<Block> inventoryBlocks = new ArrayList<>();

        // Mods register a block's item form as `register_item(same_id, name, tooltip)`
        // alongside `register_block(same_id)` — BlockDef itself carries neither. Collect
        // those ids up front so the item loop below hands their name/tooltip to the
        // block's YogBlockItem instead of ALSO registering a second, non-block Item
        // under the same Identifier (that used to silently collide in Registries.ITEM
        // and show up as a duplicate, unplaceable ghost entry in the creative tab).
        Set<String> blockIds = new HashSet<>();
        YogContent content = YogContent.cached(NativeBridge::nativeContent);
        for (YogContent.Block b : content.blocks) blockIds.add(b.id());
        Map<String, String> blockItemNames = new HashMap<>();
        Map<String, YogContent.Item> blockItemDefs = new HashMap<>();
        Map<String, String> blockItemTooltips = new HashMap<>();

        {
            for (YogContent.Item p : content.items) {
                String id = p.id();
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;

                if (blockIds.contains(id)) {
                    blockItemNames.put(id, p.name());
                    blockItemDefs.put(id, p);
                    blockItemTooltips.put(id, p.tooltip());
                    continue;
                }

                Item.Properties settings = new Item.Properties().setId(ResourceKey.create(Registries.ITEM, ident));

                int maxDamage = p.maxDamage();
                if (maxDamage > 0) {
                    settings = settings.durability(maxDamage);
                } else {
                    settings = settings.stacksTo(p.maxStack());
                }

                if (p.fireResistant()) settings = settings.fireResistant();

                if (p.food() != null) {
                    YogContent.Food fp = p.food();
                    if (fp != null) {
                        FoodProperties.Builder fb = new FoodProperties.Builder()
                                .nutrition(fp.nutrition())
                                .saturationModifier(fp.saturation());
                    if (fp.alwaysEdible()) fb = fb.alwaysEdible();
                    FoodProperties food = fb.build();
                        settings = settings.food(food);
                    }
                }

                // Check if this item has an associated book
                // A tool, a weapon or armor of a material, else a plain (or book) item.
                Item made = YogTools.make(settings, p);
                Item item;
                if (made != null) {
                    item = made;
                } else if (p.hangs() != null) {
                    item = new YogHangingItem(settings, p.name(), p.tooltip(), p.hangs());
                } else if (p.wearable() != null) {
                    item = new YogWearableItem(settings, p.name(), p.tooltip(), p.wearable());
                } else if (p.book()) {
                    item = new YogBookItem(settings,
                            p.name(), p.tooltip(), id);
                } else {
                    item = new YogItem(settings,
                            p.name(), p.tooltip());
                }
                Registry.register(BuiltInRegistries.ITEM, ident, item);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(item);

                int fuelTicks = p.fuelTicks();
                if (fuelTicks > 0) FUELS.put(item, fuelTicks);
            }
        }

        {
            for (YogContent.Block p : content.blocks) {
                String id = p.id();
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;
                float hardness   = p.hardness();
                float resistance = p.resistance();

                BlockBehaviour.Properties settings = BlockBehaviour.Properties.of()
                        .setId(ResourceKey.create(Registries.BLOCK, ident))
                        .strength(hardness, resistance);

                if (p.light() > 0) {
                    int lv = p.light();
                    settings = settings.lightLevel(state -> lv);
                }
                if (p.sound() != null) {
                    settings = settings.sound(blockSoundGroup(p.sound()));
                }
                if (p.requiresTool()) settings = settings.requiresCorrectToolForDrops();
                if (p.noCollision())  settings = settings.noCollision();
                if (p.noOcclusion())  settings = settings.noOcclusion();
                if (p.slipperiness() > 0) {
                    settings = settings.friction(p.slipperiness());
                }

                Block block;
                if (!p.kind().isEmpty()) {
                    block = YogBlockKinds.make(settings, p, baseId -> {
                        Identifier r = Identifier.tryParse(baseId);
                        return r == null ? null : BuiltInRegistries.BLOCK.getOptional(r).orElse(null);
                    });
                } else if (p.inventory() != null) {
                    YogInventoryDef def = INVENTORY_DEFS.get(p.inventory());
                    int slotCount = def != null ? def.slotCount : 0;
                    block = new YogInventoryBlock(settings, p.inventory(), slotCount);
                    inventoryBlocks.add(block);
                } else if (p.redstone()) {
                    block = new YogRedstoneBlock(settings);
                } else if (p.connects()) {
                    double[] core = {6, 0, 6, 10, 16, 10};
                    if (p.shape() != null) {
                        float[] sp = p.shape();
                        for (int i = 0; i < 6; i++) core[i] = sp[i];
                    }
                    block = new YogConnectingBlock(settings, core[0], core[1], core[2], core[3], core[4], core[5]);
                } else if (p.stateful()) {
                    block = YogStateBlock.create(settings, p);
                } else if (p.shape() != null) {
                    float[] sp = p.shape();
                    block = new YogShapedBlock(settings,
                            sp[0], sp[1],
                            sp[2], sp[3],
                            sp[4], sp[5]);
                } else {
                    block = new Block(settings);
                }

                Registry.register(BuiltInRegistries.BLOCK, ident, block);
                if (!p.connectGroups().isEmpty()) {
                    YogConnectingLogic.registerGroups(block, p.connectGroups().toArray(new String[0]));
                }
                Item blockItem = new YogBlockItem(block, blockItemSettings(ident, blockItemDefs.get(id)),
                        blockItemNames.getOrDefault(id, ""),
                        blockItemTooltips.getOrDefault(id, ""));
                Registry.register(BuiltInRegistries.ITEM, ident, blockItem);
                YogContent.Item ownItem = blockItemDefs.get(id);
                if (ownItem != null && ownItem.fuelTicks() > 0) FUELS.put(blockItem, ownItem.fuelTicks());
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(blockItem);
            }
        }

        // One generic block entity type valid for every inventory-backed block,
        // and one generic menu type for every InventoryDef (see yog-inventory's DESIGN.md).
        if (!inventoryBlocks.isEmpty()) {
            INVENTORY_BLOCK_ENTITY_TYPE = Registry.register(BuiltInRegistries.BLOCK_ENTITY_TYPE,
                    Identifier.fromNamespaceAndPath("yog", "inventory"),
                    new BlockEntityType<YogInventoryBlockEntity>(
                            (pos, state) -> {
                                YogInventoryBlock ib = (YogInventoryBlock) state.getBlock();
                                return new YogInventoryBlockEntity(pos, state, ib.defId(), ib.slotCount());
                            },
                            java.util.Set.copyOf(inventoryBlocks)));
        }
        INVENTORY_SCREEN_HANDLER_TYPE = Registry.register(BuiltInRegistries.MENU,
                Identifier.fromNamespaceAndPath("yog", "inventory"),
                new ExtendedMenuType<>(YogInventoryMenu::createClient, ByteBufCodecs.STRING_UTF8));

        // Create one creative tab per namespace, using the first item from each as icon.
        for (Map.Entry<String, List<ItemLike>> entry : tabGroups.entrySet()) {
            String ns = entry.getKey();
            List<ItemLike> entries = entry.getValue();
            if (entries.isEmpty()) continue;

            ItemLike icon = entries.get(0);
            CreativeModeTab group = FabricCreativeModeTab.builder()
                    .icon(() -> new ItemStack(icon))
                    .title(Component.literal(ns))
                    .displayItems((displayContext, tabEntries) -> {
                        entries.forEach(tabEntries::accept);
                        YogEffects.addTabStacks(ns, tabEntries::accept);
                    })
                    .build();
            Registry.register(BuiltInRegistries.CREATIVE_MODE_TAB, Identifier.fromNamespaceAndPath(ns, ns), group);
        }

        YogEntityTypes.eggs();
    }
    /** A sound group from its sound event ids: the registered events, else made by id (a mod's, or one this version lacks). */
    private static SoundType blockSoundGroup(YogContent.Sound s) {
        return new SoundType(s.volume(), s.pitch(), soundEvent(s.breakSound()), soundEvent(s.step()),
                soundEvent(s.place()), soundEvent(s.hit()), soundEvent(s.fall()));
    }

    static SoundEvent soundEvent(String id) {
        Identifier i = Identifier.tryParse(id);
        if (i == null) i = Identifier.tryParse("minecraft:intentionally_empty");
        SoundEvent e = BuiltInRegistries.SOUND_EVENT.getValue(i);
        return e != null ? e : SoundEvent.createVariableRangeEvent(i);
    }

    /**
     * Build a Brigadier literal node with typed argument chain from a schema like
     * {@code ["int", "player", "blockpos"]}.  Each arg is named {@code arg_N}.
     * All resolved args are serialised tab-separated and forwarded to Rust.
     */
    private static LiteralArgumentBuilder<CommandSourceStack>
            buildTypedCommand(String name, String[] schema) {
        var root = Commands.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, "", ctx));
            return root;
        }
        // Build argument chain from last to first, wrapping inner with outer.
        ArgumentBuilder<CommandSourceStack, ?> chain = buildLeaf(name, schema, schema.length - 1);
        for (int i = schema.length - 2; i >= 0; i--) {
            chain = buildArgNode(schema[i], argName(i, schema[i])).then(chain);
            // Also allow executing with fewer args (partial match not standard; just attach executes at leaf).
        }
        root.then(chain);
        return root;
    }

    /**
     * A typed argument's node name — its position and type, so schemas that
     * differ there get separate Brigadier branches instead of being merged
     * into the first one's (Brigadier merges same-named nodes).
     */
    private static String argName(int index, String type) {
        return "arg_" + index + "_" + type;
    }

    /** Schema lines with greedy `string` args last: Brigadier tries a command's branches in registration order, so word chains go first. */
    private static java.util.List<String> typedSchemaOrder(String lines) {
        java.util.List<String> out = new java.util.ArrayList<>(java.util.Arrays.asList(lines.split("\n")));
        out.sort(java.util.Comparator.comparing(l -> l.contains("string")));
        return out;
    }

    private static RequiredArgumentBuilder<CommandSourceStack, ?> buildArgNode(String type, String argName) {
        return switch (type) {
            case "int"      -> Commands.argument(argName, IntegerArgumentType.integer());
            case "float"    -> Commands.argument(argName, FloatArgumentType.floatArg());
            case "word"     -> Commands.argument(argName, StringArgumentType.word());
            case "string"   -> Commands.argument(argName, StringArgumentType.greedyString());
            case "player"   -> Commands.argument(argName, EntityArgument.player());
            case "blockpos" -> Commands.argument(argName, BlockPosArgument.blockPos());
            default         -> Commands.argument(argName, StringArgumentType.word());
        };
    }

    private static ArgumentBuilder<CommandSourceStack, ?> buildLeaf(String cmdName, String[] schema, int idx) {
        String type    = schema[idx];
        String argName = argName(idx, type);
        return buildArgNode(type, argName).executes(ctx -> {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i <= idx; i++) {
                if (i > 0) sb.append('\t');
                sb.append(resolveArg(schema[i], argName(i, schema[i]), ctx));
            }
            return runCommand(cmdName, sb.toString(), ctx);
        });
    }

    private static String resolveArg(String type, String argName, CommandContext<CommandSourceStack> ctx) {
        try {
            return switch (type) {
                case "int"      -> String.valueOf(IntegerArgumentType.getInteger(ctx, argName));
                case "float"    -> String.valueOf(FloatArgumentType.getFloat(ctx, argName));
                case "word", "string" -> StringArgumentType.getString(ctx, argName);
                case "player"   -> EntityArgument.getPlayer(ctx, argName).getName().getString();
                case "blockpos" -> {
                    net.minecraft.core.BlockPos pos = BlockPosArgument.getBlockPos(ctx, argName);
                    yield pos.getX() + "," + pos.getY() + "," + pos.getZ();
                }
                default -> StringArgumentType.getString(ctx, argName);
            };
        } catch (Exception e) {
            return "";
        }
    }

    private static int runCommand(String name, String args, CommandContext<CommandSourceStack> ctx) {
        CommandSourceStack src = ctx.getSource();
        net.minecraft.world.entity.Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getStringUUID() : "";
        String dim = src.getLevel().dimension().identifier().toString();
        String reply = NativeBridge.nativeOnCommand(name, args, src.getTextName(), uuid, dim);
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }

    /** Everything entity {@code about} keeps, to {@code to}. */
    private static void sendEntityData(net.minecraft.server.level.ServerPlayer to, net.minecraft.world.entity.Entity about) {
        byte[] data = NativeBridge.nativeEntityDataFor(about.getStringUUID());
        if (data != null) NativeBridge.sendToPlayer(to.getName().getString(), "yog:entity_data", data);
    }

    /**
     * A block's item form as the mod's {@code .yblock} says ({@code item}:
     * how it stacks, whether it survives fire); the game's defaults otherwise.
     */
    private static Item.Properties blockItemSettings(Identifier id, YogContent.Item own) {
        Item.Properties s = new Item.Properties().setId(ResourceKey.create(Registries.ITEM, id)).useBlockDescriptionPrefix();
        if (own == null) return s;
        s = s.stacksTo(own.maxStack());
        return own.fireResistant() ? s.fireResistant() : s;
    }
}
