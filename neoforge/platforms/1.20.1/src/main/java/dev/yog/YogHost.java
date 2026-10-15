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
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.inventory.MenuType;
import net.minecraftforge.network.IContainerFactory;

import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.EntityArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.sounds.SoundEvent;
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
import net.minecraft.world.level.Level;
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
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
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
    private final List<Block> inventoryBlocks = new ArrayList<>();

    // ── yog-inventory (see rust/crates/yog-inventory/DESIGN.md) ────────────────

    /** Parsed `InventoryDef`s, keyed by id — shared by the block entity/menu/screen. */
    public static final Map<String, YogInventoryDef> INVENTORY_DEFS = new HashMap<>();
    /** One generic block entity type valid for every block with an `inventory_id`. */
    public static BlockEntityType<YogInventoryBlockEntity> INVENTORY_BLOCK_ENTITY_TYPE;
    /** One generic menu type for every `InventoryDef` — the def id is synced as the
     *  screen-opening data so the client knows which layout/slot-count to build. */
    public static MenuType<YogInventoryMenu> INVENTORY_MENU_TYPE;

    private static void parseInventoryDefs() {
        INVENTORY_DEFS.putAll(YogContent.cached(NativeBridge::nativeContent).inventories);
    }

    public YogHost() {
        NativeBridge.ensureLoaded();
        System.out.println("[yog] NeoForge host initialised.");

        YogNetworkBridge.init();
        parseInventoryDefs();

        var modBus = FMLJavaModLoadingContext.get().getModEventBus();
        modBus.addListener(this::onRegister);
        modBus.addListener(this::onAddPackFinders);

        MinecraftForge.EVENT_BUS.register(this);
    }

    // ── Content registration (mod bus) ───────────────────────────────────────

    private void onRegister(RegisterEvent event) {
        if (event.getRegistryKey().equals(Registries.BLOCK)) {
            registerBlocks(event);
        } else if (event.getRegistryKey().equals(Registries.ENTITY_TYPE)) {
            YogEntityTypes.build(YogContent.cached(NativeBridge::nativeContent).entities,
                    (id, type) -> event.register(Registries.ENTITY_TYPE, id, () -> type));
        } else if (event.getRegistryKey().equals(Registries.ITEM)) {
            registerItems(event);
            YogEntityTypes.eggs((id, item) -> event.register(Registries.ITEM, id, () -> item));
        } else if (event.getRegistryKey().equals(Registries.CREATIVE_MODE_TAB)) {
            registerTabs(event);
        } else if (event.getRegistryKey().equals(Registries.BLOCK_ENTITY_TYPE)) {
            registerInventoryBlockEntityType(event);
        } else if (event.getRegistryKey().equals(Registries.MENU)) {
            registerInventoryMenuType(event);
        } else if (event.getRegistryKey().equals(Registries.CHUNK_GENERATOR)) {
            registerChunkGenerators(event);
        }
    }

    private void registerChunkGenerators(RegisterEvent event) {
        event.register(Registries.CHUNK_GENERATOR, new ResourceLocation("yog", "callback_generator"),
                () -> YogCallbackChunkGenerator.CODEC);
    }

    private void onAddPackFinders(AddPackFindersEvent event) {
        event.addRepositorySource(new YogPackProvider(event.getPackType()));
    }

    @SuppressWarnings("unchecked")
    private void registerInventoryBlockEntityType(RegisterEvent event) {
        if (inventoryBlocks.isEmpty()) return;
        INVENTORY_BLOCK_ENTITY_TYPE = BlockEntityType.Builder.of(
                (pos, state) -> {
                    YogInventoryBlock ib = (YogInventoryBlock) state.getBlock();
                    return new YogInventoryBlockEntity(pos, state, ib.defId(), ib.slotCount());
                },
                inventoryBlocks.toArray(new Block[0])
        ).build(null);
        event.register(Registries.BLOCK_ENTITY_TYPE, new ResourceLocation("yog", "inventory"),
                () -> INVENTORY_BLOCK_ENTITY_TYPE);
    }

    private void registerInventoryMenuType(RegisterEvent event) {
        INVENTORY_MENU_TYPE = net.minecraftforge.common.extensions.IForgeMenuType.create(
                (IContainerFactory<YogInventoryMenu>) YogInventoryMenu::createClient);
        event.register(Registries.MENU, new ResourceLocation("yog", "inventory"),
                () -> INVENTORY_MENU_TYPE);
    }

    private void registerBlocks(RegisterEvent event) {
        for (YogContent.Block p : YogContent.cached(NativeBridge::nativeContent).blocks) {
            String id = p.id();
            ResourceLocation ident = ResourceLocation.tryParse(id);
            if (ident == null) continue;
            float hardness   = p.hardness();
            float resistance = p.resistance();

            BlockBehaviour.Properties props = BlockBehaviour.Properties.of()
                    .strength(hardness, resistance);

            if (p.light() > 0) {
                int lv = p.light();
                props = props.lightLevel(state -> lv);
            }
            if (p.sound() != null) {
                props = props.sound(soundType(p.sound()));
            }
            if (p.requiresTool()) props = props.requiresCorrectToolForDrops();
            if (p.noCollision()) props = props.noCollission();
            if (p.slipperiness() > 0) {
                props = props.friction(p.slipperiness());
            }

            Block block;
            if (p.inventory() != null) {
                YogInventoryDef def = INVENTORY_DEFS.get(p.inventory());
                int slotCount = def != null ? def.slotCount : 0;
                block = new YogInventoryBlock(props, p.inventory(), slotCount);
                inventoryBlocks.add(block);
            } else if (p.redstone()) {
                block = new YogRedstoneBlock(props);
            } else if (p.connects()) {
                double[] core = {6, 0, 6, 10, 16, 10};
                if (p.shape() != null) {
                    float[] sp = p.shape();
                    for (int i = 0; i < 6; i++) core[i] = sp[i];
                }
                block = new YogConnectingBlock(props, core[0], core[1], core[2], core[3], core[4], core[5]);
            } else if (p.stateful()) {
                block = YogStateBlock.create(props, p);
            } else if (p.shape() != null) {
                float[] sp = p.shape();
                block = new YogShapedBlock(props,
                        sp[0], sp[1],
                        sp[2], sp[3],
                        sp[4], sp[5]);
            } else {
                block = new Block(props);
            }

            event.register(Registries.BLOCK, ident, () -> block);
            registeredBlocks.put(ident, block);
            if (!p.connectGroups().isEmpty()) {
                YogConnectingLogic.registerGroups(block, p.connectGroups().toArray(new String[0]));
            }
        }
    }

    // name/tooltip for ids that turn out to be blocks, pulled from the matching
    // register_item(...) call — mods use `ItemDef::new(block_id).name(..).tooltip(..)`
    // as the block's display metadata (BlockDef itself carries neither yet).
    private final Map<String, String> blockItemNames = new HashMap<>();
    private final Map<String, String> blockItemTooltips = new HashMap<>();

    private void registerItems(RegisterEvent event) {
        YogContent content = YogContent.cached(NativeBridge::nativeContent);
        {
            for (YogContent.Item p : content.items) {
                String id = p.id();
                ResourceLocation ident = ResourceLocation.tryParse(id);
                if (ident == null) continue;

                if (registeredBlocks.containsKey(ident)) {
                    // This id belongs to a block — don't register a second, competing
                    // Item under the same id (that used to silently collide and show up
                    // as a duplicate, unplaceable ghost entry in the creative tab). Just
                    // hand its name/tooltip to the block-item loop below.
                    blockItemNames.put(id, p.name());
                    blockItemTooltips.put(id, p.tooltip());
                    continue;
                }

                Item.Properties props = new Item.Properties();

                int maxDamage = p.maxDamage();
                if (maxDamage > 0) {
                    props = props.durability(maxDamage);
                } else {
                    props = props.stacksTo(p.maxStack());
                }

                if (p.fireResistant()) props = props.fireResistant();

                if (p.food() != null) {
                    YogContent.Food fp = p.food();
                    if (fp != null) {
                        FoodProperties.Builder fb = new FoodProperties.Builder()
                                .nutrition(fp.nutrition())
                                .saturationMod(fp.saturation());
                        if (fp.alwaysEdible())
                            fb = fb.alwaysEat();
                        props = props.food(fb.build());
                    }
                }
                // A tool, a weapon or armor of a material, else a plain (or book) item.
                Item made = YogTools.make(props, p);
                Item item;
                if (made != null) {
                    item = made;
                } else if (p.wearable() != null) {
                    item = new YogWearableItem(props, p.name(), p.tooltip(), p.wearable());
                } else if (p.book()) {
                    item = new YogBookItem(props,
                            p.name(), p.tooltip(), id);
                } else {
                    item = new YogItem(props,
                            p.name(), p.tooltip());
                }
                event.register(Registries.ITEM, ident, () -> item);
                tabGroups.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(item);

                int fuelTicks = p.fuelTicks();
                if (fuelTicks > 0) FUEL.put(item, fuelTicks);
            }
        }

        // Block items for blocks registered in the BLOCK phase.
        {
            for (YogContent.Block p : YogContent.cached(NativeBridge::nativeContent).blocks) {
                String id = p.id();
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
                    NativeBridge.nativeOnPlayerJoin(found.getName().getString(), uuid,
                            found.level().dimension().location().toString());
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

        // Plain commands
        String names = NativeBridge.nativeCommandNames();
        if (names == null || names.isBlank()) return;
        for (String name : names.split("\n")) {
            if (name.isBlank()) continue;
            dispatcher.register(Commands.literal(name)
                    .executes(ctx -> runCommand(name, "", ctx))
                    .then(Commands.argument("args", StringArgumentType.greedyString())
                            .executes(ctx -> runCommand(name, StringArgumentType.getString(ctx, "args").trim().replaceAll("\\s+", "\t"), ctx))));
        }
    }

    // ── Block break ──────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onBlockBreak(BlockEvent.BreakEvent event) {
        if (!(event.getPlayer() instanceof ServerPlayer player)) return;
        String blockId = BuiltInRegistries.BLOCK.getKey(event.getState().getBlock()).toString();
        String playerName = player.getName().getString();
        int x = event.getPos().getX(), y = event.getPos().getY(), z = event.getPos().getZ();
        String dim = ((Level) event.getLevel()).dimension().location().toString();
        if (!NativeBridge.nativeOnBlockBreakPre(playerName, blockId, x, y, z, dim)) {
            event.setCanceled(true);
            return;
        }
        // Defer Post to after the block is actually removed.
        // BlockEvent.BreakEvent fires *before* the break — if we setBlock
        // here, vanilla will destroy the block we just placed.
        // NB: server.execute() runs inline when already on the server thread
        // (BlockableEventLoop), so it would NOT defer — queue for end of tick.
        POST_TICK.add(() -> NativeBridge.nativeOnBlockBreak(playerName, blockId, x, y, z, dim));
    }

    // ── Chat ─────────────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onChat(ServerChatEvent event) {
        String playerName = event.getPlayer().getName().getString();
        String message = event.getMessage().getString();
        String dim = event.getPlayer().level().dimension().location().toString();
        if (!NativeBridge.nativeOnChatPre(playerName, message, dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnChat(playerName, message, dim);
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
        NativeBridge.nativeOnPlayerLeave(player.getName().getString(), pUuid,
                player.level().dimension().location().toString());
    }

    // ── Right-click item ─────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onRightClickItem(PlayerInteractEvent.RightClickItem event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        ItemStack stack = sp.getItemInHand(event.getHand());
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        String dim = sp.level().dimension().location().toString();
        if (!NativeBridge.nativeOnUseItemPre(sp.getName().getString(), itemId, sp.isShiftKeyDown(), dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId, sp.isShiftKeyDown(), dim);
    }

    // ── Right-click block ────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onRightClickBlock(PlayerInteractEvent.RightClickBlock event) {
        if (event.getSide() != LogicalSide.SERVER) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        // As vanilla: sneaking with something in hand uses the item (places a block), not the block.
        boolean useBlock = !(sp.isSecondaryUseActive() && (!sp.getMainHandItem().isEmpty() || !sp.getOffhandItem().isEmpty()));
        BlockPos pos = event.getPos();
        String blockId = BuiltInRegistries.BLOCK.getKey(sp.level().getBlockState(pos).getBlock()).toString();
        String dim = sp.level().dimension().location().toString();
        if (useBlock) {
            if (!NativeBridge.nativeOnUseBlockPre(sp.getName().getString(), blockId,
                    pos.getX(), pos.getY(), pos.getZ(), dim, sp.isShiftKeyDown())) {
                event.setCanceled(true);
                return;
            }
            NativeBridge.nativeOnUseBlock(sp.getName().getString(), blockId,
                    pos.getX(), pos.getY(), pos.getZ(), dim, sp.isShiftKeyDown());
        }

        // Block placement — Pre (cancellable)
        ItemStack held = sp.getItemInHand(event.getHand());
        if (held.getItem() instanceof BlockItem bi) {
            BlockPos placed = pos.relative(event.getFace());
            String bid = BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
            if (!NativeBridge.nativeOnPlaceBlockPre(
                    sp.getName().getString(), bid, placed.getX(), placed.getY(), placed.getZ(), dim, sp.isShiftKeyDown())) {
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
        String dim = sp.level().dimension().location().toString();
        if (!NativeBridge.nativeOnEntityInteractPre(pName, pUuid, eType, eUuid, handStr, dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityInteract(pName, pUuid, eType, eUuid, handStr, dim);
    }

    // ── Attack entity ────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onAttackEntity(AttackEntityEvent event) {
        if (event.getEntity().level().isClientSide) return;
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        Entity target = event.getTarget();
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).toString();
        NativeBridge.nativeOnAttackEntity(sp.getName().getString(), type, target.getStringUUID(),
                sp.level().dimension().location().toString());
    }

    // ── Entity damage / player death ─────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onLivingDamage(LivingDamageEvent event) {
        if (event.getEntity().level().isClientSide) return;
        LivingEntity entity = event.getEntity();
        String source = event.getSource().getMsgId();
        String dim = entity.level().dimension().location().toString();

        if (entity instanceof ServerPlayer sp && sp.getHealth() - event.getAmount() <= 0.0f) {
            // Damage that would kill the player — Pre (cancellable).
            boolean allow = NativeBridge.nativeOnPlayerDeathPre(
                    sp.getName().getString(), sp.getStringUUID(), source, dim);
            if (!allow) {
                event.setCanceled(true);
                return;
            }
        }

        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        if (!NativeBridge.nativeOnEntityDamagePre(
                type, entity.getStringUUID(), event.getAmount(), source, dim)) {
            event.setCanceled(true);
            return;
        }
        NativeBridge.nativeOnEntityDamage(
                type, entity.getStringUUID(), event.getAmount(), source, dim);
    }

    // ── Entity spawn ────────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onEntityJoinLevel(EntityJoinLevelEvent event) {
        if (event.getLevel().isClientSide()) return;
        Entity entity = event.getEntity();
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        String uuid = entity.getStringUUID();
        String dim = ((Level) event.getLevel()).dimension().location().toString();
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
        String dim = entity.level().dimension().location().toString();
        if (entity instanceof ServerPlayer sp) {
            NativeBridge.nativeOnPlayerDeath(
                    sp.getName().getString(), sp.getStringUUID(), source, dim);
            return;
        }
        String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        NativeBridge.nativeOnEntityDeath(type, entity.getStringUUID(), source, dim);
    }

    // ── Player respawn ───────────────────────────────────────────────────────

    @net.minecraftforge.eventbus.api.SubscribeEvent
    public void onPlayerRespawn(PlayerEvent.PlayerRespawnEvent event) {
        if (!(event.getEntity() instanceof ServerPlayer sp)) return;
        if (event.isEndConquered()) return; // dimension-change respawn, not death
        NativeBridge.nativeOnPlayerRespawn(
                sp.getName().getString(), sp.getStringUUID(), false,
                sp.level().dimension().location().toString());
    }

    // ── helpers ──────────────────────────────────────────────────────────────
    /** A sound group from its sound event ids: the registered events, else made by id (a mod's, or one this version lacks). */
    private static SoundType soundType(YogContent.Sound s) {
        return new SoundType(s.volume(), s.pitch(), soundEvent(s.breakSound()), soundEvent(s.step()),
                soundEvent(s.place()), soundEvent(s.hit()), soundEvent(s.fall()));
    }

    static SoundEvent soundEvent(String id) {
        ResourceLocation rl = ResourceLocation.tryParse(id);
        if (rl == null) rl = ResourceLocation.tryParse("minecraft:intentionally_empty");
        SoundEvent e = BuiltInRegistries.SOUND_EVENT.get(rl);
        return e != null ? e : SoundEvent.createVariableRangeEvent(rl);
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
            chain = buildArgNode(schema[i], argName(i, schema[i])).then(chain);
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
        String dim = src.getLevel().dimension().location().toString();
        String reply = NativeBridge.nativeOnCommand(name, args, src.getTextName(), uuid, dim);
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }
}
