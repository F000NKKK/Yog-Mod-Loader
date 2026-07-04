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
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.fml.LogicalSide;
import net.neoforged.fml.common.Mod;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.neoforge.event.RegisterCommandsEvent;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;
import net.neoforged.neoforge.event.entity.living.LivingDamageEvent;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;
import net.neoforged.neoforge.event.entity.living.LivingEvent;
import net.neoforged.neoforge.event.entity.player.AttackEntityEvent;
import net.neoforged.neoforge.event.entity.player.PlayerEvent;
import net.neoforged.neoforge.event.entity.player.PlayerInteractEvent;
import net.neoforged.neoforge.event.level.BlockEvent;
import net.neoforged.neoforge.event.ServerChatEvent;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.server.ServerStoppingEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;
import net.neoforged.neoforge.network.event.RegisterPayloadHandlerEvent;
import net.neoforged.neoforge.network.PacketDistributor;

import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.item.BlockItem;
import net.minecraft.item.CreativeModeTab;
import net.minecraft.item.FoodProperties;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.command.ServerCommandSource;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.sounds.BlockSoundGroup;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.network.chat.Component;
import net.minecraft.core.BlockPos;

/**
 * NeoForge entry point. Boots the native Yog runtime and forwards events
 * to it via {@link NativeBridge}. "The Gate and the Key."
 */
@Mod("yog")
public class YogHost {
    /** UUID → [name, retriesLeft] for players waiting to appear in the server list. */
    private static final java.util.concurrent.ConcurrentHashMap<String, String[]> PENDING_JOINS =
            new java.util.concurrent.ConcurrentHashMap<>();

    public YogHost(IEventBus modBus) {
        NativeBridge.ensureLoaded();
        System.out.println("[yog] NeoForge host initialised.");

        // Register mod-declared content now, before the registries freeze.
        registerContent();

        // Register event handlers on the Forge event bus
        NeoForge.EVENT_BUS.register(this);
    }

    // ── Server lifecycle ─────────────────────────────────────────────────────

    @SubscribeEvent
    public void onServerStarted(ServerStartedEvent event) {
        NativeBridge.setServer(event.getServer());
        String worldDir = event.getServer()
                .getSavePath(net.minecraft.util.WorldSavePath.ROOT)
                .toAbsolutePath().toString();
        NativeBridge.nativeOnServerStarted(worldDir);
    }

    @SubscribeEvent
    public void onServerStopping(ServerStoppingEvent event) {
        NativeBridge.nativeOnServerStopping();
    }

    // ── Server tick ──────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onServerTick(ServerTickEvent event) {
        if (event.phase != ServerTickEvent.Phase.END) return;
        MinecraftServer server = event.getServer();

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

    @SubscribeEvent
    public void onRegisterCommands(RegisterCommandsEvent event) {
        var dispatcher = event.getDispatcher();
        var registryAccess = event.getBuildContext();

        // Typed commands
        java.util.Map<String, String> typedSchemas = new java.util.HashMap<>();
        String schemaLines = NativeBridge.nativeTypedCommandSchemas();
        if (schemaLines != null) {
            for (String line : schemaLines.split("\n")) {
                if (line.isBlank()) continue;
                int tab = line.indexOf('\t');
                if (tab > 0) typedSchemas.put(line.substring(0, tab), line.substring(tab + 1));
            }
        }
        for (java.util.Map.Entry<String, String> e : typedSchemas.entrySet()) {
            String name = e.getKey();
            String schema = e.getValue();
            dispatcher.register(buildTypedCommand(name, schema.split("\\s+")));
        }

        // Plain commands
        String names = NativeBridge.nativeCommandNames();
        if (names == null || names.isBlank()) return;
        for (String name : names.split("\n")) {
            if (name.isBlank() || typedSchemas.containsKey(name)) continue;
            dispatcher.register(CommandManager.literal(name)
                    .executes(ctx -> runCommand(name, "", ctx))
                    .then(CommandManager.argument("args", StringArgumentType.greedyString())
                            .executes(ctx -> runCommand(name, StringArgumentType.getString(ctx, "args"), ctx))));
        }
    }

    // ── Block break ──────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onBlockBreak(BlockEvent.BreakEvent event) {
        ServerPlayer player = (ServerPlayer) event.getPlayer();
        String blockId = Registries.BLOCK.getKey(event.getState().getBlock()).toString();
        if (!NativeBridge.nativeOnBlockBreakPre(
                player.getName().getString(), blockId,
                event.getPos().getX(), event.getPos().getY(), event.getPos().getZ())) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnBlockBreak(
                player.getName().getString(), blockId,
                event.getPos().getX(), event.getPos().getY(), event.getPos().getZ());
    }

    // ── Chat ─────────────────────────────────────────────────────────────────

    @SubscribeEvent
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

    @SubscribeEvent
    public void onPlayerJoin(PlayerEvent.PlayerLoggedInEvent event) {
        ServerPlayer player = (ServerPlayer) event.getEntity();
        String pUuid = player.getStringUUID();
        String pName = player.getName().getString();
        PENDING_JOINS.put(pUuid, new String[]{pName, "40"});
    }

    @SubscribeEvent
    public void onPlayerLeave(PlayerEvent.PlayerLoggedOutEvent event) {
        ServerPlayer player = (ServerPlayer) event.getEntity();
        String pUuid = player.getStringUUID();
        PENDING_JOINS.remove(pUuid);
        NativeBridge.nativeOnPlayerLeave(player.getName().getString(), pUuid);
    }

    // ── Right-click item ─────────────────────────────────────────────────────

    @SubscribeEvent
    public void onRightClickItem(PlayerInteractEvent.RightClickItem event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        ServerPlayer sp = (ServerPlayer) event.getEntity();
        ItemStack stack = sp.getItemInHand(event.getHand());
        String itemId = Registries.ITEM.getKey(stack.getItem()).toString();
        NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId);
    }

    // ── Right-click block ────────────────────────────────────────────────────

    @SubscribeEvent
    public void onRightClickBlock(PlayerInteractEvent.RightClickBlock event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        ServerPlayer sp = (ServerPlayer) event.getEntity();
        BlockPos pos = event.getPos();
        String blockId = Registries.BLOCK.getKey(sp.level().getBlockState(pos).getBlock()).toString();
        NativeBridge.nativeOnUseBlock(sp.getName().getString(), blockId,
                pos.getX(), pos.getY(), pos.getZ());

        // Block placement — Pre (cancellable)
        ItemStack held = sp.getItemInHand(event.getHand());
        if (held.getItem() instanceof BlockItem bi) {
            BlockPos placed = pos.relative(event.getFace());
            String bid = Registries.BLOCK.getKey(bi.getBlock()).toString();
            if (!NativeBridge.nativeOnPlaceBlockPre(
                    sp.getName().getString(), bid, placed.getX(), placed.getY(), placed.getZ())) {
                event.setCanceled(true);
            }
        }
    }

    // ── Entity interact ──────────────────────────────────────────────────────

    @SubscribeEvent
    public void onEntityInteract(PlayerInteractEvent.EntityInteract event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        ServerPlayer sp = (ServerPlayer) event.getEntity();
        Entity target = event.getTarget();
        String pName = sp.getName().getString();
        String pUuid = sp.getStringUUID();
        String eType = Registries.ENTITY_TYPE.getKey(target.getType()).toString();
        String eUuid = target.getStringUUID();
        String handStr = event.getHand() == InteractionHand.MAIN_HAND ? "main_hand" : "off_hand";
        if (!NativeBridge.nativeOnEntityInteractPre(pName, pUuid, eType, eUuid, handStr)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityInteract(pName, pUuid, eType, eUuid, handStr);
    }

    // ── Attack entity ────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onAttackEntity(AttackEntityEvent event) {
        if (event.getEntity().level().isClientSide) return;
        ServerPlayer sp = (ServerPlayer) event.getEntity();
        Entity target = event.getTarget();
        String type = Registries.ENTITY_TYPE.getKey(target.getType()).toString();
        NativeBridge.nativeOnAttackEntity(sp.getName().getString(), type, target.getStringUUID());
    }

    // ── Entity damage ───────────────────────────────────────────────────────

    @SubscribeEvent
    public void onLivingDamage(LivingDamageEvent event) {
        if (event.getEntity().level().isClientSide) return;
        LivingEntity entity = event.getEntity();
        String type = Registries.ENTITY_TYPE.getKey(entity.getType()).toString();
        String source = event.getSource().getMsgId();
        if (!NativeBridge.nativeOnEntityDamagePre(
                type, entity.getStringUUID(), event.getAmount(), source)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityDamage(
                type, entity.getStringUUID(), event.getAmount(), source);
    }

    // ── Entity spawn ────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onEntityJoinLevel(EntityJoinLevelEvent event) {
        if (event.getLevel().isClientSide()) return;
        Entity entity = event.getEntity();
        String type = Registries.ENTITY_TYPE.getKey(entity.getType()).toString();
        String uuid = entity.getStringUUID();
        String dim = event.getLevel().dimension().location().toString();
        if (!NativeBridge.nativeOnEntitySpawnPre(type, uuid, dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntitySpawn(type, uuid, dim);
    }

    // ── Entity death ────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onLivingDeath(LivingDeathEvent event) {
        if (event.getEntity().level().isClientSide) return;
        LivingEntity entity = event.getEntity();
        if (entity instanceof ServerPlayer) return; // handled below
        String type = Registries.ENTITY_TYPE.getKey(entity.getType()).toString();
        NativeBridge.nativeOnEntityDeath(type, entity.getStringUUID(), event.getSource().getMsgId());
    }

    // ── Player death ────────────────────────────────────────────────────────

    @SubscribeEvent
    public void onLivingDamage_PlayerDeath(LivingDamageEvent event) {
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        if (event.getEntity().level().isClientSide) return;
        String source = event.getSource().getMsgId();
        boolean allow = NativeBridge.nativeOnPlayerDeathPre(
                sp.getName().getString(), sp.getStringUUID(), source);
        if (!allow) {
            sp.setHealth(0.5f); // prevent death as expected by ABI
            event.setCanceled(true);
            return;
        }
        // Post is fire-and-forget; actual death still happens.
        NativeBridge.nativeOnPlayerDeath(
                sp.getName().getString(), sp.getStringUUID(), source);
    }

    // ── Player respawn ───────────────────────────────────────────────────────

    @SubscribeEvent
    public void onPlayerRespawn(PlayerEvent.PlayerRespawnEvent event) {
        ServerPlayer sp = (ServerPlayer) event.getEntity();
        if (event.isEndConquered()) return; // dimension-change respawn, not death
        NativeBridge.nativeOnPlayerRespawn(
                sp.getName().getString(), sp.getStringUUID(), false);
    }

    // ── Content registration helpers ────────────────────────────────────────

    private static Map<String, String> parseProps(String line) {
        String[] parts = line.split("\t", -1);
        Map<String, String> props = new HashMap<>();
        for (int i = 1; i < parts.length; i++) {
            int eq = parts[i].indexOf('=');
            if (eq > 0) props.put(parts[i].substring(0, eq), parts[i].substring(eq + 1));
        }
        return props;
    }

    private static void registerContent() {
        Map<String, List<net.minecraft.world.level.ItemLike>> tabGroups = new LinkedHashMap<>();

        String items = NativeBridge.nativeItemDefs();
        if (items != null) {
            for (String line : items.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                ResourceLocation ident = ResourceLocation.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = parseProps(line);
                Item.Properties settings = new Item.Properties();

                int maxDamage = parseInt(p, "max_damage", 0);
                if (maxDamage > 0) {
                    settings = settings.durability(maxDamage);
                } else {
                    settings = settings.stacksTo(parseInt(p, "max_stack", 64));
                }

                if ("1".equals(p.get("fire_resistant"))) settings = settings.fireResistant();

                if (p.containsKey("food")) {
                    String[] fp = p.get("food").split(":", 3);
                    if (fp.length >= 2) {
                        FoodProperties.Builder fb = new FoodProperties.Builder()
                                .nutrition(Integer.parseInt(fp[0]))
                                .saturationMod(Float.parseFloat(fp[1]));
                        if ("1".equals(fp.length > 2 ? fp[2] : "0"))
                            fb = fb.alwaysEat();
                        settings = settings.food(fb.build());
                    }
                }

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

                int fuelTicks = parseInt(p, "fuel_ticks", 0);
                if (fuelTicks > 0) {
                    net.neoforged.neoforge.common.extensions.IItemExtension ext =
                            (net.neoforged.neoforge.common.extensions.IItemExtension) item;
                    net.neoforged.neoforge.common.extensions.IItemExtension.getBurnTime(item.getDefaultInstance(), null);
                }
            }
        }

        String blocks = NativeBridge.nativeBlockDefs();
        if (blocks != null) {
            for (String line : blocks.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                ResourceLocation ident = ResourceLocation.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = parseProps(line);
                float hardness = parseFloat(p, "hardness", 1.5f);
                float resistance = parseFloat(p, "resistance", 6.0f);

                AbstractBlock.Properties settings = AbstractBlock.Properties.of()
                        .strength(hardness, resistance);

                if (p.containsKey("light")) {
                    int lv = parseInt(p, "light", 0);
                    settings = settings.lightLevel(state -> lv);
                }
                if (p.containsKey("sound")) {
                    settings = settings.sound(blockSoundGroup(p.get("sound")));
                }
                if ("1".equals(p.get("requires_tool"))) settings = settings.requiresCorrectToolForDrops();
                if ("1".equals(p.get("no_collision"))) settings = settings.noCollission();
                if (p.containsKey("slipperiness")) {
                    settings = settings.friction(parseFloat(p, "slipperiness", 0.6f));
                }

                Block block;
                if (p.containsKey("shape")) {
                    String[] sp = p.get("shape").split(":", 6);
                    block = new YogShapedBlock(settings,
                            Double.parseDouble(sp[0]), Double.parseDouble(sp[1]),
                            Double.parseDouble(sp[2]), Double.parseDouble(sp[3]),
                            Double.parseDouble(sp[4]), Double.parseDouble(sp[5]));
                } else {
                    block = new Block(settings);
                }

                Registry.register(Registries.BLOCK, ident, block);
                Item blockItem = new YogBlockItem(block, new Item.Properties(),
                        p.getOrDefault("name", ""));
                Registry.register(Registries.ITEM, ident, blockItem);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(blockItem);
            }
        }

        // Create one creative tab per namespace
        for (Map.Entry<String, List<net.minecraft.world.level.ItemLike>> entry : tabGroups.entrySet()) {
            String ns = entry.getKey();
            List<net.minecraft.world.level.ItemLike> entries = entry.getValue();
            if (entries.isEmpty()) continue;

            net.minecraft.world.level.ItemLike icon = entries.get(0);
            CreativeModeTab group = CreativeModeTab.builder()
                    .icon(() -> new ItemStack(icon))
                    .title(Component.literal(ns))
                    .displayItems((params, output) -> entries.forEach(output::accept))
                    .build();
            Registry.register(Registries.CREATIVE_MODE_TAB, new ResourceLocation(ns, ns), group);
        }
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

    private static BlockSoundGroup blockSoundGroup(String name) {
        return switch (name) {
            case "wood" -> BlockSoundGroup.WOOD;
            case "grass" -> BlockSoundGroup.GRASS;
            case "gravel" -> BlockSoundGroup.GRAVEL;
            case "snow" -> BlockSoundGroup.SNOW;
            case "sand" -> BlockSoundGroup.SAND;
            case "metal" -> BlockSoundGroup.METAL;
            case "glass" -> BlockSoundGroup.GLASS;
            case "wool" -> BlockSoundGroup.WOOL;
            case "nether_brick" -> BlockSoundGroup.NETHER_BRICKS;
            default -> BlockSoundGroup.STONE;
        };
    }

    // ── Brigadier command builder (unchanged from Fabric) ────────────────────

    private static LiteralArgumentBuilder<ServerCommandSource>
            buildTypedCommand(String name, String[] schema) {
        var root = CommandManager.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, "", ctx));
            return root;
        }
        ArgumentBuilder<ServerCommandSource, ?> chain = buildLeaf(name, schema, schema.length - 1);
        for (int i = schema.length - 2; i >= 0; i--) {
            chain = buildArgNode(schema[i], "arg_" + i).then(chain);
        }
        root.then(chain);
        return root;
    }

    private static RequiredArgumentBuilder<ServerCommandSource, ?> buildArgNode(String type, String argName) {
        return switch (type) {
            case "int" -> CommandManager.argument(argName, IntegerArgumentType.integer());
            case "float" -> CommandManager.argument(argName, FloatArgumentType.floatArg());
            case "word" -> CommandManager.argument(argName, StringArgumentType.word());
            case "string" -> CommandManager.argument(argName, StringArgumentType.greedyString());
            case "player" -> CommandManager.argument(argName, EntityArgumentType.player());
            case "blockpos" -> CommandManager.argument(argName, BlockPosArgumentType.blockPos());
            default -> CommandManager.argument(argName, StringArgumentType.word());
        };
    }

    private static ArgumentBuilder<ServerCommandSource, ?> buildLeaf(String cmdName, String[] schema, int idx) {
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

    private static String resolveArg(String type, String argName, CommandContext<ServerCommandSource> ctx) {
        try {
            return switch (type) {
                case "int" -> String.valueOf(IntegerArgumentType.getInteger(ctx, argName));
                case "float" -> String.valueOf(FloatArgumentType.getFloat(ctx, argName));
                case "word", "string" -> StringArgumentType.getString(ctx, argName);
                case "player" -> EntityArgumentType.getPlayer(ctx, argName).getName().getString();
                case "blockpos" -> {
                    BlockPos pos = BlockPosArgumentType.getBlockPos(ctx, argName);
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
        Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getStringUUID() : "";
        String reply = NativeBridge.nativeOnCommand(name, args, src.getTextName(), uuid);
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }
}