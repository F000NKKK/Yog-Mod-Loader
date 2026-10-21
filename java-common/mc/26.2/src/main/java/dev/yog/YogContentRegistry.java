package dev.yog;

import dev.yog.block.YogConnectingLogic;
import dev.yog.data.YogContent;
import dev.yog.util.YogInventoryDef;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.ItemLike;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;

/**
 * The mods' content — items, blocks, their inventories, creative tabs,
 * entity types, effects — made from the content blob and handed to
 * {@link YogRegistrar}, the same on every loader. What a loader feeds to
 * its own hooks (fuel, default attributes, brewing) is kept here for it.
 */
public final class YogContentRegistry {
    private YogContentRegistry() {}

    /** Every mod's inventory, by id: the block entity, the menu and the screen share them. */
    public static final Map<String, YogInventoryDef> INVENTORY_DEFS = new HashMap<>();
    /** One block entity type for every block with an inventory. */
    public static BlockEntityType<YogInventoryBlockEntity> INVENTORY_BLOCK_ENTITY_TYPE;
    /** One menu type for every inventory: the inventory's id comes with the screen it opens. */
    public static MenuType<YogInventoryMenu> INVENTORY_MENU_TYPE;
    /** Burn time in ticks of each item the mods made fuel, for the loader's fuel hook. */
    public static final Map<Item, Integer> FUELS = new LinkedHashMap<>();

    /** Where Yog's own types (inventories) are registered: {@code yog:inventory}. */
    private static final Identifier INVENTORY = Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, "inventory");
    /** A connecting block's core when its {@code .yblock} gives no shape: a post in the middle, pixels. */
    private static final double[] DEFAULT_CORE = {6, 0, 6, 10, 16, 10};
    /** The sound when a mod names none (or one that isn't): silence. */
    private static final String NO_SOUND = "minecraft:intentionally_empty";

    /** The blocks made, by id: their items need them (a later register event, on NeoForge). */
    private static final Map<String, Block> BLOCKS = new LinkedHashMap<>();
    private static final List<Block> INVENTORY_BLOCKS = new ArrayList<>();
    /** The items of each namespace, for its creative tab. */
    private static final Map<String, List<ItemLike>> TABS = new LinkedHashMap<>();

    /** Schedules every mod's content with {@link YogRegistrar}. Called once, before the registries freeze. */
    public static void schedule() {
        YogContent content = YogContent.cached(NativeBridge::nativeContent);
        INVENTORY_DEFS.putAll(content.inventories);
        YogCallbackChunkGenerator.schedule();
        YogEffects.schedule(content);
        // Mods' entity types first: spawn eggs (items) name them.
        YogEntityTypes.schedule(content.entities);
        YogRegistrar.add(Registries.BLOCK, sink -> blocks(content, sink));
        YogRegistrar.add(Registries.ITEM, sink -> items(content, sink));
        YogEntityTypes.scheduleEggs();
        YogRegistrar.add(Registries.BLOCK_ENTITY_TYPE, YogContentRegistry::inventoryBlockEntity);
        YogRegistrar.add(Registries.MENU, sink -> {
            INVENTORY_MENU_TYPE = YogPlatform.inventoryMenuType();
            sink.register(Registries.MENU, INVENTORY, INVENTORY_MENU_TYPE);
        });
        YogRegistrar.add(Registries.CREATIVE_MODE_TAB, YogContentRegistry::tabs);
    }

    private static void blocks(YogContent content, YogRegistrar.Sink sink) {
        for (YogContent.Block p : content.blocks) {
            Identifier ident = Identifier.tryParse(p.id());
            if (ident == null) continue;
            Block block = block(p, ident);
            sink.register(Registries.BLOCK, ident, block);
            BLOCKS.put(p.id(), block);
            if (!p.connectGroups().isEmpty()) {
                YogConnectingLogic.registerGroups(block, p.connectGroups().toArray(new String[0]));
            }
        }
    }

    private static Block block(YogContent.Block p, Identifier ident) {
        BlockBehaviour.Properties settings = BlockBehaviour.Properties.of()
                .setId(ResourceKey.create(Registries.BLOCK, ident))
                .strength(p.hardness(), p.resistance());
        if (p.light() > 0) {
            int lv = p.light();
            settings = settings.lightLevel(state -> lv);
        }
        if (p.sound() != null) settings = settings.sound(blockSoundGroup(p.sound()));
        if (p.requiresTool()) settings = settings.requiresCorrectToolForDrops();
        if (p.noCollision()) settings = settings.noCollision();
        if (p.noOcclusion()) settings = settings.noOcclusion();
        if (p.slipperiness() > 0) settings = settings.friction(p.slipperiness());

        if (!p.kind().isEmpty()) {
            return YogBlockKinds.make(settings, p, baseId -> {
                Identifier r = Identifier.tryParse(baseId);
                return r == null ? null : BLOCKS.getOrDefault(baseId, BuiltInRegistries.BLOCK.getOptional(r).orElse(null));
            });
        }
        if (p.inventory() != null) {
            YogInventoryDef def = INVENTORY_DEFS.get(p.inventory());
            Block block = new YogInventoryBlock(settings, p.inventory(), def != null ? def.slotCount : 0);
            INVENTORY_BLOCKS.add(block);
            return block;
        }
        if (p.redstone()) return new YogRedstoneBlock(settings);
        if (p.connects()) {
            double[] core = DEFAULT_CORE.clone();
            if (p.shape() != null) {
                float[] sp = p.shape();
                for (int i = 0; i < core.length; i++) core[i] = sp[i];
            }
            return new YogConnectingBlock(settings, core);
        }
        if (p.stateful()) return YogStateBlock.create(settings, p);
        if (p.shape() != null) {
            return new YogShapedBlock(settings, p.shape());
        }
        return new Block(settings);
    }

    /**
     * The mods' items, and their blocks' item forms: a block's {@code .yitem}
     * of the same id names and shapes its item (one item, not two).
     */
    private static void items(YogContent content, YogRegistrar.Sink sink) {
        Set<String> blockIds = new HashSet<>();
        for (YogContent.Block b : content.blocks) blockIds.add(b.id());
        Map<String, YogContent.Item> blockItems = new HashMap<>();
        for (YogContent.Item p : content.items) {
            Identifier ident = Identifier.tryParse(p.id());
            if (ident == null) continue;
            if (blockIds.contains(p.id())) {
                blockItems.put(p.id(), p);
                continue;
            }
            Item item = item(p, ident);
            sink.register(Registries.ITEM, ident, item);
            TABS.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(item);
            if (p.fuelTicks() > 0) FUELS.put(item, p.fuelTicks());
        }
        for (Map.Entry<String, Block> b : BLOCKS.entrySet()) {
            Identifier ident = Identifier.parse(b.getKey());
            YogContent.Item own = blockItems.get(b.getKey());
            Item blockItem = new YogBlockItem(b.getValue(), blockItemSettings(ident, own),
                    own != null ? own.name() : "", own != null ? own.tooltip() : "");
            sink.register(Registries.ITEM, ident, blockItem);
            if (own != null && own.fuelTicks() > 0) FUELS.put(blockItem, own.fuelTicks());
            TABS.computeIfAbsent(ident.getNamespace(), k -> new ArrayList<>()).add(blockItem);
        }
    }

    private static Item item(YogContent.Item p, Identifier ident) {
        Item.Properties settings = new Item.Properties().setId(ResourceKey.create(Registries.ITEM, ident));
        settings = p.maxDamage() > 0 ? settings.durability(p.maxDamage()) : settings.stacksTo(p.maxStack());
        if (p.fireResistant()) settings = settings.fireResistant();
        if (p.food() != null) {
            YogContent.Food fp = p.food();
            FoodProperties.Builder fb = new FoodProperties.Builder().nutrition(fp.nutrition()).saturationModifier(fp.saturation());
            if (fp.alwaysEdible()) fb = fb.alwaysEdible();
            settings = settings.food(fb.build());
        }
        // A tool, a weapon or armor of a material, else what it says it is.
        Item made = YogTools.make(settings, p);
        if (made != null) return made;
        if (p.hangs() != null) return new YogHangingItem(settings, p.name(), p.tooltip(), p.hangs());
        if (p.wearable() != null) return new YogWearableItem(settings, p.name(), p.tooltip(), p.wearable());
        if (p.book()) return new YogBookItem(settings, p.name(), p.tooltip(), p.id());
        return new YogItem(settings, p.name(), p.tooltip());
    }

    /** One block entity type valid for every block with an inventory. */
    private static void inventoryBlockEntity(YogRegistrar.Sink sink) {
        if (INVENTORY_BLOCKS.isEmpty()) return;
        INVENTORY_BLOCK_ENTITY_TYPE = new BlockEntityType<>(
                (pos, state) -> {
                    YogInventoryBlock ib = (YogInventoryBlock) state.getBlock();
                    return new YogInventoryBlockEntity(pos, state, ib.defId(), ib.slotCount());
                },
                Set.copyOf(INVENTORY_BLOCKS));
        sink.register(Registries.BLOCK_ENTITY_TYPE, INVENTORY, INVENTORY_BLOCK_ENTITY_TYPE);
    }

    /** One creative tab per namespace, its first item its icon. */
    private static void tabs(YogRegistrar.Sink sink) {
        for (Map.Entry<String, List<ItemLike>> entry : TABS.entrySet()) {
            String ns = entry.getKey();
            List<ItemLike> entries = entry.getValue();
            if (entries.isEmpty()) continue;
            ItemLike icon = entries.get(0);
            CreativeModeTab tab = YogPlatform.tabBuilder()
                    .icon(() -> new ItemStack(icon))
                    .title(Component.literal(ns))
                    .displayItems((params, out) -> {
                        entries.forEach(out::accept);
                        YogEffects.addTabStacks(ns, out::accept);
                    })
                    .build();
            sink.register(Registries.CREATIVE_MODE_TAB, Identifier.fromNamespaceAndPath(ns, ns), tab);
        }
    }

    /** A sound group from its sound event ids: the registered events, else made by id (a mod's, or one this version lacks). */
    private static SoundType blockSoundGroup(YogContent.Sound s) {
        return new SoundType(s.volume(), s.pitch(), soundEvent(s.breakSound()), soundEvent(s.step()),
                soundEvent(s.place()), soundEvent(s.hit()), soundEvent(s.fall()));
    }

    static SoundEvent soundEvent(String id) {
        Identifier i = Identifier.tryParse(id);
        if (i == null) i = Identifier.parse(NO_SOUND);
        SoundEvent e = BuiltInRegistries.SOUND_EVENT.getValue(i);
        return e != null ? e : SoundEvent.createVariableRangeEvent(i);
    }

    /** A block's item form as its {@code .yitem} says (how it stacks, whether it survives fire); the game's defaults otherwise. */
    private static Item.Properties blockItemSettings(Identifier id, YogContent.Item own) {
        Item.Properties s = new Item.Properties().setId(ResourceKey.create(Registries.ITEM, id)).useBlockDescriptionPrefix();
        if (own == null) return s;
        s = s.stacksTo(own.maxStack());
        return own.fireResistant() ? s.fireResistant() : s;
    }
}
