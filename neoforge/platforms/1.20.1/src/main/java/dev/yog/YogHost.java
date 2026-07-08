package dev.yog;

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
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.EntityArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.ItemLike;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.state.BlockBehaviour;

import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.AddPackFindersEvent;
import net.minecraftforge.event.RegisterCommandsEvent;
import net.minecraftforge.event.ServerChatEvent;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.event.entity.EntityJoinLevelEvent;
import net.minecraftforge.event.entity.living.LivingDamageEvent;
import net.minecraftforge.event.entity.living.LivingDeathEvent;
import net.minecraftforge.event.entity.player.AttackEntityEvent;
import net.minecraftforge.event.entity.player.PlayerEvent;
import net.minecraftforge.event.entity.player.PlayerInteractEvent;
import net.minecraftforge.event.furnace.FurnaceFuelBurnTimeEvent;
import net.minecraftforge.event.level.BlockEvent;
import net.minecraftforge.event.server.ServerStartedEvent;
import net.minecraftforge.event.server.ServerStoppingEvent;
import net.minecraftforge.fml.LogicalSide;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.registries.RegisterEvent;
import net.minecraftforge.server.ServerLifecycleHooks;

/**
 * NeoForge entry point. Boots the native Yog runtime and forwards events
 * to it via {@link NativeBridge}. "The Gate and the Key."
 */
@Mod("yog")
public class YogHost {
    /** Callbacks deferred to the end of the current server tick. */
    private static final java.util.concurrent.ConcurrentLinkedQueue<Runnable> POST_TICK =
            new java.util.concurrent.ConcurrentLinkedQueue<>();

    /** UUID → [name, retriesLeft] for players waiting to appear in the server list. */
    private static final java.util.concurrent.ConcurrentHashMap<String, String[]> PENDING_JOINS =
            new java.util.concurrent.ConcurrentHashMap<>();

    /** Item → burn time in ticks for mod-registered fuels. */
    private static final Map<Item, Integer> FUEL = new HashMap<>();

    // Parsed content defs, shared between RegisterEvent phases.
    private final Map<ResourceLocation, Block> registeredBlocks = new LinkedHashMap<>();
    private final Map<String, List<ItemLike>> tabGroups = new LinkedHashMap<>();

    public YogHost() {
        NativeBridge.ensureLoaded();
        System.out.println("[yog] NeoForge host initialised.");

        var modBus = FMLJavaModLoadingContext.get().getModEventBus();
        modBus.addListener(this::onRegister);
        modBus.addListener(this::onAddPackFinders);

        MinecraftForge.EVENT_BUS.register(this);
    }

    // ── Content registration (mod bus) ───────────────────────────────────────

    private void onRegister(RegisterEvent event) {
        if (event.getRegistryKey().equals(Registries.BLOCK)) {
            registerBlocks(event);
        } else if (event.getRegistryKey().equals(Registries.ITEM)) {
            registerItems(event);
        } else if (event.getRegistryKey().equals(Registries.CREATIVE_MODE_TAB)) {
            registerTabs(event);
        }
    }

    private void onAddPackFinders(AddPackFindersEvent event) {
        event.addRepositorySource(new YogPackProvider(event.getPackType()));
    }

    private void registerBlocks(RegisterEvent event) {
        String blocks = NativeBridge.nativeBlockDefs();
        if (blocks == null) return;
        for (String line : blocks.split("\n")) {
            if (line.isBlank()) continue;
            String id = line.split("\t", 2)[0];
            ResourceLocation ident = ResourceLocation.tryParse(id);
            if (ident == null) continue;

            Map<String, String> p = parseProps(line);
            float hardness   = parseFloat(p, "hardness", 1.5f);
            float resistance = parseFloat(p, "resistance", 6.0f);

            BlockBehaviour.Properties props = BlockBehaviour.Properties.of()
                    .strength(hardness, resistance);

            if (p.containsKey("light")) {
                int lv = parseInt(p, "light", 0);
                props = props.lightLevel(state -> lv);
            }
            if (p.containsKey("sound")) {
                props = props.sound(soundType(p.get("sound")));
            }
            if ("1".equals(p.get("requires_tool"))) props = props.requiresCorrectToolForDrops();
            if ("1".equals(p.get("no_collision"))) props = props.noCollission();
            if (p.containsKey("slipperiness")) {
                props = props.friction(parseFloat(p, "slipperiness", 0.6f));
            }

            Block block;
            if ("1".equals(p.get("connects"))) {
                double[] core = {6, 0, 6, 10, 16, 10};
                if (p.containsKey("shape")) {
                    String[] sp = p.get("shape").split(":", 6);
                    for (int i = 0; i < 6; i++) core[i] = Double.parseDouble(sp[i]);
                }
                block = new YogConnectingBlock(props, core[0], core[1], core[2], core[3], core[4], core[5]);
            } else if (p.containsKey("shape")) {
                String[] sp = p.get("shape").split(":", 6);
                block = new YogShapedBlock(props,
                        Double.parseDouble(sp[0]), Double.parseDouble(sp[1]),
                        Double.parseDouble(sp[2]), Double.parseDouble(sp[3]),
                        Double.parseDouble(sp[4]), Double.parseDouble(sp[5]));
            } else {
                block = new Block(props);
            }

            event.register(Registries.BLOCK, ident, () -> block);
            registeredBlocks.put(ident, block);
            if (p.containsKey("connect_groups")) {
                YogConnectingLogic.registerGroups(block, p.get("connect_groups").split(","));
            }
        }
    }

    // name/tooltip for ids that turn out to be blocks, pulled from the matching
    // register_item(...) call — mods use `ItemDef::new(block_id).name(..).tooltip(..)`
    // as the block's display metadata (BlockDef itself carries neither yet).
    private final Map<String, String> blockItemNames = new HashMap<>();
    private final Map<String, String> blockItemTooltips = new HashMap<>();

    private void registerItems(RegisterEvent event) {
        String items = NativeBridge.nativeItemDefs();
        if (items != null) {
            for (String line : items.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                ResourceLocation ident = ResourceLocation.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = parseProps(line);

                if (registeredBlocks.containsKey(ident)) {
                    // This id belongs to a block — don't register a second, competing
                    // Item under the same id (that used to silently collide and show up
                    // as a duplicate, unplaceable ghost entry in the creative tab). Just
                    // hand its name/tooltip to the block-item loop below.
                    blockItemNames.put(id, p.getOrDefault("name", ""));
                    blockItemTooltips.put(id, p.getOrDefault("tooltip", ""));
                    continue;
                }

                Item.Properties props = new Item.Properties();

                int maxDamage = parseInt(p, "max_damage", 0);
                if (maxDamage > 0) {
                    props = props.durability(maxDamage);
                } else {
                    props = props.stacksTo(parseInt(p, "max_stack", 64));
                }

                if ("1".equals(p.get("fire_resistant"))) props = props.fireResistant();

                if (p.containsKey("food")) {
                    String[] fp = p.get("food").split(":", 3);
                    if (fp.length >= 2) {
                        FoodProperties.Builder fb = new FoodProperties.Builder()
                                .nutrition(Integer.parseInt(fp[0]))
                                .saturationMod(Float.parseFloat(fp[1]));
                        if ("1".equals(fp.length > 2 ? fp[2] : "0"))
                            fb = fb.alwaysEat();
                        props = props.food(fb.build());
                    }
                }

                String bookJson = NativeBridge.nativeBookJson(id);
                Item item;
                if (bookJson != null && !bookJson.equals("null")) {
                    item = new YogBookItem(props,
                            p.getOrDefault("name", ""), p.getOrDefault("tooltip", ""), id);
                } else {
                    item = new YogItem(props,
                            p.getOrDefault("name", ""), p.getOrDefault("tooltip", ""));
                }
                event.register(Registries.ITEM, ident, () -> item);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(item);

                int fuelTicks = parseInt(p, "fuel_ticks", 0);
                if (fuelTicks > 0) FUEL.put(item, fuelTicks);
            }
        }

        // Block items for blocks registered in the BLOCK phase.
        String blocks = NativeBridge.nativeBlockDefs();
        if (blocks != null) {
            for (String line : blocks.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                ResourceLocation ident = ResourceLocation.tryParse(id);
                Block block = ident == null ? null : registeredBlocks.get(ident);
                if (block == null) continue;

                Item blockItem = new YogBlockItem(block, new Item.Properties(),
                        blockItemNames.getOrDefault(id, ""),
                        blockItemTooltips.getOrDefault(id, ""));
                event.register(Registries.ITEM, ident, () -> blockItem);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(blockItem);
            }
        }
    }

    private void registerTabs(RegisterEvent event) {
        for (Map.Entry<String, List<ItemLike>> entry : tabGroups.entrySet()) {
            String ns = entry.getKey();
            List<ItemLike> entries = entry.getValue();
            if (entries.isEmpty()) continue;

            ItemLike icon = entries.get(0);
            CreativeModeTab tab = CreativeModeTab.builder()
                    .icon(() -> new ItemStack(icon))
                    .title(Component.literal(ns))
                    .displayItems((params, output) -> entries.forEach(output::accept))
                    .build();
            event.register(Registries.CREATIVE_MODE_TAB, new ResourceLocation(ns, ns), () -> tab);
        }
    }

    // ── Fuel burn time (mod-registered fuels) ────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onFuelBurnTime(FurnaceFuelBurnTimeEvent event) {
        Integer ticks = FUEL.get(event.getItemStack().getItem());
        if (ticks != null) event.setBurnTime(ticks);
    }

    // ── Server lifecycle ─────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onServerStarted(ServerStartedEvent event) {
        NativeBridge.setServer(event.getServer());
        String worldDir = event.getServer()
                .getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT)
                .toAbsolutePath().toString();
        NativeBridge.nativeOnServerStarted(worldDir);
    }

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onServerStopping(ServerStoppingEvent event) {
        NativeBridge.nativeOnServerStopping();
    }

    // ── Server tick ──────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onServerTick(TickEvent.ServerTickEvent event) {
        if (event.phase != TickEvent.Phase.END) return;
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        if (server == null) return;

        // Deferred callbacks queued by pre-events (e.g. block-break post).
        Runnable deferred;
        while ((deferred = POST_TICK.poll()) != null) deferred.run();

        // Resolve pending player joins
        if (!PENDING_JOINS.isEmpty()) {
            java.util.List<String> toRemove = new java.util.ArrayList<>();
            PENDING_JOINS.forEach((uuid, entry) -> {
                String name = entry[0];
                int retries = Integer.parseInt(entry[1]);
                java.util.UUID parsed = java.util.UUID.fromString(uuid);
                ServerPlayer found = server.getPlayerList().getPlayer(parsed);
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
    }

    // ── Commands ─────────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onRegisterCommands(RegisterCommandsEvent event) {
        var dispatcher = event.getDispatcher();

        // Typed commands. A single command name can have MULTIPLE distinct
        // schemas (subcommands with different arg counts) — each is registered
        // separately; Brigadier merges repeated `dispatcher.register()` calls
        // that share a root literal name into one command-tree node, so this
        // produces one node with a sibling branch per distinct arg shape,
        // instead of collapsing them all down to whichever schema was seen last.
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

        // Plain commands
        String names = NativeBridge.nativeCommandNames();
        if (names == null || names.isBlank()) return;
        for (String name : names.split("\n")) {
            if (name.isBlank()) continue;
            dispatcher.register(Commands.literal(name)
                    .executes(ctx -> runCommand(name, "", ctx))
                    .then(Commands.argument("args", StringArgumentType.greedyString())
                            .executes(ctx -> runCommand(name, StringArgumentType.getString(ctx, "args"), ctx))));
        }
    }

    // ── Block break ──────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onBlockBreak(BlockEvent.BreakEvent event) {
        if (!(event.getPlayer() instanceof ServerPlayer player)) return;
        String blockId = BuiltInRegistries.BLOCK.getKey(event.getState().getBlock()).toString();
        String playerName = player.getName().getString();
        int x = event.getPos().getX(), y = event.getPos().getY(), z = event.getPos().getZ();
        if (!NativeBridge.nativeOnBlockBreakPre(playerName, blockId, x, y, z)) {
            event.setCanceled(true);
            return;
        }
        // Defer Post to after the block is actually removed.
        // BlockEvent.BreakEvent fires *before* the break — if we setBlock
        // here, vanilla will destroy the block we just placed.
        // NB: server.execute() runs inline when already on the server thread
        // (BlockableEventLoop), so it would NOT defer — queue for end of tick.
        POST_TICK.add(() -> NativeBridge.nativeOnBlockBreak(playerName, blockId, x, y, z));
    }

    // ── Chat ─────────────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onChat(ServerChatEvent event) {
        String playerName = event.getPlayer().getName().getString();
        String message = event.getMessage().getString();
        if (!NativeBridge.nativeOnChatPre(playerName, message)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnChat(playerName, message);
    }

    // ── Player join / leave ──────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onPlayerJoin(PlayerEvent.PlayerLoggedInEvent event) {
        if (!(event.getEntity() instanceof ServerPlayer player)) return;
        String pUuid = player.getStringUUID();
        String pName = player.getName().getString();
        PENDING_JOINS.put(pUuid, new String[]{pName, "40"});
    }

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onPlayerLeave(PlayerEvent.PlayerLoggedOutEvent event) {
        if (!(event.getEntity() instanceof ServerPlayer player)) return;
        String pUuid = player.getStringUUID();
        PENDING_JOINS.remove(pUuid);
        NativeBridge.nativeOnPlayerLeave(player.getName().getString(), pUuid);
    }

    // ── Right-click item ─────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onRightClickItem(PlayerInteractEvent.RightClickItem event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        ItemStack stack = sp.getItemInHand(event.getHand());
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId, sp.isShiftKeyDown());
    }

    // ── Right-click block ────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onRightClickBlock(PlayerInteractEvent.RightClickBlock event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        BlockPos pos = event.getPos();
        String blockId = BuiltInRegistries.BLOCK.getKey(sp.level().getBlockState(pos).getBlock()).toString();
        NativeBridge.nativeOnUseBlock(sp.getName().getString(), blockId,
                pos.getX(), pos.getY(), pos.getZ());

        // Block placement — Pre (cancellable)
        ItemStack held = sp.getItemInHand(event.getHand());
        if (held.getItem() instanceof BlockItem bi) {
            BlockPos placed = pos.relative(event.getFace());
            String bid = BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
            if (!NativeBridge.nativeOnPlaceBlockPre(
                    sp.getName().getString(), bid, placed.getX(), placed.getY(), placed.getZ())) {
                event.setCanceled(true);
            }
        }
    }

    // ── Entity interact ──────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onEntityInteract(PlayerInteractEvent.EntityInteract event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        Entity target = event.getTarget();
        String pName = sp.getName().getString();
        String pUuid = sp.getStringUUID();
        String eType = BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).toString();
        String eUuid = target.getStringUUID();
        String handStr = event.getHand() == InteractionHand.MAIN_HAND ? "main_hand" : "off_hand";
        if (!NativeBridge.nativeOnEntityInteractPre(pName, pUuid, eType, eUuid, handStr)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityInteract(pName, pUuid, eType, eUuid, handStr);
    }

    // ── Attack entity ────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onAttackEntity(AttackEntityEvent event) {
        if (event.getEntity().level().isClientSide) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        Entity target = event.getTarget();
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).toString();
        NativeBridge.nativeOnAttackEntity(sp.getName().getString(), type, target.getStringUUID());
    }

    // ── Entity damage / player death ─────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onLivingDamage(LivingDamageEvent event) {
        if (event.getEntity().level().isClientSide) return;
        LivingEntity entity = event.getEntity();
        String source = event.getSource().getMsgId();

        if (entity instanceof ServerPlayer sp && sp.getHealth() - event.getAmount() <= 0.0f) {
            // Damage that would kill the player — Pre (cancellable).
            boolean allow = NativeBridge.nativeOnPlayerDeathPre(
                    sp.getName().getString(), sp.getStringUUID(), source);
            if (!allow) {
                event.setCanceled(true);
                return;
            }
        }

        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        if (!NativeBridge.nativeOnEntityDamagePre(
                type, entity.getStringUUID(), event.getAmount(), source)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityDamage(
                type, entity.getStringUUID(), event.getAmount(), source);
    }

    // ── Entity spawn ────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onEntityJoinLevel(EntityJoinLevelEvent event) {
        if (event.getLevel().isClientSide()) return;
        Entity entity = event.getEntity();
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        String uuid = entity.getStringUUID();
        String dim = event.getLevel().dimension().location().toString();
        if (!NativeBridge.nativeOnEntitySpawnPre(type, uuid, dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntitySpawn(type, uuid, dim);
    }

    // ── Entity / player death ────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onLivingDeath(LivingDeathEvent event) {
        if (event.getEntity().level().isClientSide) return;
        LivingEntity entity = event.getEntity();
        String source = event.getSource().getMsgId();
        if (entity instanceof ServerPlayer sp) {
            NativeBridge.nativeOnPlayerDeath(
                    sp.getName().getString(), sp.getStringUUID(), source);
            return;
        }
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        NativeBridge.nativeOnEntityDeath(type, entity.getStringUUID(), source);
    }

    // ── Player respawn ───────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onPlayerRespawn(PlayerEvent.PlayerRespawnEvent event) {
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        if (event.isEndConquered()) return; // dimension-change respawn, not death
        NativeBridge.nativeOnPlayerRespawn(
                sp.getName().getString(), sp.getStringUUID(), false);
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    private static Map<String, String> parseProps(String line) {
        String[] parts = line.split("\t", -1);
        Map<String, String> props = new HashMap<>();
        for (int i = 1; i < parts.length; i++) {
            int eq = parts[i].indexOf('=');
            if (eq > 0) props.put(parts[i].substring(0, eq), parts[i].substring(eq + 1));
        }
        return props;
    }

    private static int parseInt(Map<String, String> p, String key, int def) {
        String v = p.get(key);
        if (v == null) return def;
        try { return Integer.parseInt(v); } catch (NumberFormatException e) { return def; }
    }

    private static float parseFloat(Map<String, String> p, String key, float def) {
        String v = p.get(key);
        if (v == null) return def;
        try { return Float.parseFloat(v); } catch (NumberFormatException e) { return def; }
    }

    private static SoundType soundType(String name) {
        return switch (name) {
            case "wood" -> SoundType.WOOD;
            case "grass" -> SoundType.GRASS;
            case "gravel" -> SoundType.GRAVEL;
            case "snow" -> SoundType.SNOW;
            case "sand" -> SoundType.SAND;
            case "metal" -> SoundType.METAL;
            case "glass" -> SoundType.GLASS;
            case "wool" -> SoundType.WOOL;
            case "nether_brick" -> SoundType.NETHER_BRICKS;
            default -> SoundType.STONE;
        };
    }

    // ── Brigadier command builder (same shape as the Fabric host) ────────────

    private static LiteralArgumentBuilder<CommandSourceStack>
            buildTypedCommand(String name, String[] schema) {
        var root = Commands.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, "", ctx));
            return root;
        }
        ArgumentBuilder<CommandSourceStack, ?> chain = buildLeaf(name, schema, schema.length - 1);
        for (int i = schema.length - 2; i >= 0; i--) {
            chain = buildArgNode(schema[i], "arg_" + i).then(chain);
        }
        root.then(chain);
        return root;
    }

    private static RequiredArgumentBuilder<CommandSourceStack, ?> buildArgNode(String type, String argName) {
        return switch (type) {
            case "int" -> Commands.argument(argName, IntegerArgumentType.integer());
            case "float" -> Commands.argument(argName, FloatArgumentType.floatArg());
            case "word" -> Commands.argument(argName, StringArgumentType.word());
            case "string" -> Commands.argument(argName, StringArgumentType.greedyString());
            case "player" -> Commands.argument(argName, EntityArgument.player());
            case "blockpos" -> Commands.argument(argName, BlockPosArgument.blockPos());
            default -> Commands.argument(argName, StringArgumentType.word());
        };
    }

    private static ArgumentBuilder<CommandSourceStack, ?> buildLeaf(String cmdName, String[] schema, int idx) {
        String type = schema[idx];
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

    private static String resolveArg(String type, String argName, CommandContext<CommandSourceStack> ctx) {
        try {
            return switch (type) {
                case "int" -> String.valueOf(IntegerArgumentType.getInteger(ctx, argName));
                case "float" -> String.valueOf(FloatArgumentType.getFloat(ctx, argName));
                case "word", "string" -> StringArgumentType.getString(ctx, argName);
                case "player" -> EntityArgument.getPlayer(ctx, argName).getName().getString();
                case "blockpos" -> {
                    BlockPos pos = BlockPosArgument.getBlockPos(ctx, argName);
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
        Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getStringUUID() : "";
        String reply = NativeBridge.nativeOnCommand(name, args, src.getTextName(), uuid);
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }
}
