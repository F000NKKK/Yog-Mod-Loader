package dev.yog;

import com.mojang.brigadier.Command;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.itemgroup.v1.FabricItemGroup;
import net.fabricmc.fabric.api.registry.FuelRegistry;
import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.item.FoodComponent;
import net.minecraft.item.Item;
import net.minecraft.item.ItemConvertible;
import net.minecraft.item.ItemGroup;
import net.minecraft.registry.Registry;
import net.minecraft.sound.BlockSoundGroup;
import net.minecraft.util.Identifier;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents;
import net.fabricmc.fabric.api.event.player.AttackEntityCallback;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseBlockCallback;
import net.fabricmc.fabric.api.event.player.UseItemCallback;
import net.fabricmc.fabric.api.message.v1.ServerMessageEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.command.ServerCommandSource;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.text.Text;
import net.minecraft.util.ActionResult;
import net.minecraft.util.TypedActionResult;

/**
 * Fabric entry point. Boots the native Yog runtime and forwards server events
 * to it via {@link NativeBridge}. "The Gate and the Key."
 *
 * <p>We use Fabric API events rather than raw Mixins here: they are more stable
 * across mapping/version changes. Mixins return later for deeper hooks (e.g.
 * client rendering) that Fabric API does not cover.
 */
public class YogHost implements ModInitializer {
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
                ServerPlayNetworking.registerGlobalReceiver(id, (server, player, netHandler, buf, sender) -> {
                    byte[] data = new byte[buf.readableBytes()];
                    buf.readBytes(data);
                    server.execute(() ->
                            NativeBridge.nativeOnPacket(channel, player.getName().getString(), data));
                });
            }
        }

        // Block break (server side).
        PlayerBlockBreakEvents.AFTER.register((world, player, pos, state, blockEntity) -> {
            String blockId = Registries.BLOCK.getId(state.getBlock()).toString();
            NativeBridge.nativeOnBlockBreak(
                    player.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
        });

        // Chat.
        ServerMessageEvents.CHAT_MESSAGE.register((message, sender, params) ->
                NativeBridge.nativeOnChat(
                        sender.getName().getString(), message.getContent().getString()));

        // Player join / leave.
        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) ->
                NativeBridge.nativeOnPlayerJoin(
                        handler.player.getName().getString(), handler.player.getUuidAsString()));

        ServerPlayConnectionEvents.DISCONNECT.register((handler, server) ->
                NativeBridge.nativeOnPlayerLeave(
                        handler.player.getName().getString(), handler.player.getUuidAsString()));

        // Item use (right-click), server side only.
        UseItemCallback.EVENT.register((player, world, hand) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                ItemStack stack = sp.getStackInHand(hand);
                String itemId = Registries.ITEM.getId(stack.getItem()).toString();
                NativeBridge.nativeOnUseItem(sp.getName().getString(), itemId);
            }
            return TypedActionResult.pass(player.getStackInHand(hand));
        });

        // Block use (right-click on a block), server side only.
        UseBlockCallback.EVENT.register((player, world, hand, hitResult) -> {
            if (!world.isClient && player instanceof ServerPlayerEntity sp) {
                net.minecraft.util.math.BlockPos pos = hitResult.getBlockPos();
                String blockId = Registries.BLOCK.getId(world.getBlockState(pos).getBlock()).toString();
                NativeBridge.nativeOnUseBlock(
                        sp.getName().getString(), blockId, pos.getX(), pos.getY(), pos.getZ());
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

        // Living-entity damage (server side). Observe only; always allow.
        ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> {
            String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
            NativeBridge.nativeOnEntityDamage(
                    type, entity.getUuidAsString(), amount, source.getName());
            return true;
        });

        // Living-entity death (server side).
        ServerLivingEntityEvents.AFTER_DEATH.register((entity, source) -> {
            String type = Registries.ENTITY_TYPE.getId(entity.getType()).toString();
            NativeBridge.nativeOnEntityDeath(
                    type, entity.getUuidAsString(), source.getName());
        });

        // End-of-tick (20×/second).
        ServerTickEvents.END_SERVER_TICK.register(server -> NativeBridge.nativeOnTick());

        // Server lifecycle. Capture the server first so Rust can act on it
        // (e.g. NativeBridge.broadcast).
        ServerLifecycleEvents.SERVER_STARTED.register(server -> {
            NativeBridge.setServer(server);
            NativeBridge.nativeOnServerStarted();
        });
        ServerLifecycleEvents.SERVER_STOPPING.register(server -> NativeBridge.nativeOnServerStopping());

        // Commands: register each mod-declared command name with Brigadier and
        // route execution to Rust.
        CommandRegistrationCallback.EVENT.register((dispatcher, registryAccess, environment) -> {
            String names = NativeBridge.nativeCommandNames();
            if (names == null || names.isBlank()) {
                return;
            }
            for (String name : names.split("\n")) {
                if (name.isBlank()) {
                    continue;
                }
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
     * tooltip, and collect them into a "Yog" creative tab.
     */
    /** Parse `id\tkey=value\t...` into a map. First element is the id. */
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
        List<ItemConvertible> tabEntries = new ArrayList<>();

        String items = NativeBridge.nativeItemDefs();
        if (items != null) {
            for (String line : items.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = parseProps(line);
                Item.Settings settings = new Item.Settings();

                int maxDamage = parseInt(p, "max_damage", 0);
                if (maxDamage > 0) {
                    settings = settings.maxDamage(maxDamage);
                } else {
                    settings = settings.maxCount(parseInt(p, "max_stack", 64));
                }

                if ("1".equals(p.get("fire_resistant"))) settings = settings.fireproof();

                if (p.containsKey("food")) {
                    String[] fp = p.get("food").split(":", 3);
                    if (fp.length >= 2) {
                        FoodComponent.Builder fb = new FoodComponent.Builder()
                                .hunger(Integer.parseInt(fp[0]))
                                .saturationModifier(Float.parseFloat(fp[1]));
                    if ("1".equals(fp.length > 2 ? fp[2] : "0")) fb = fb.alwaysEdible();
                    FoodComponent food = fb.build();
                        settings = settings.food(food);
                    }
                }

                Item item = new YogItem(settings,
                        p.getOrDefault("name", ""), p.getOrDefault("tooltip", ""));
                Registry.register(Registries.ITEM, ident, item);
                tabEntries.add(item);

                int fuelTicks = parseInt(p, "fuel_ticks", 0);
                if (fuelTicks > 0) FuelRegistry.INSTANCE.add(item, fuelTicks);
            }
        }

        String blocks = NativeBridge.nativeBlockDefs();
        if (blocks != null) {
            for (String line : blocks.split("\n")) {
                if (line.isBlank()) continue;
                String id = line.split("\t", 2)[0];
                Identifier ident = Identifier.tryParse(id);
                if (ident == null) continue;

                Map<String, String> p = parseProps(line);
                float hardness   = parseFloat(p, "hardness",   1.5f);
                float resistance = parseFloat(p, "resistance",  6.0f);

                AbstractBlock.Settings settings = AbstractBlock.Settings.create()
                        .strength(hardness, resistance);

                if (p.containsKey("light")) {
                    int lv = parseInt(p, "light", 0);
                    settings = settings.luminance(state -> lv);
                }
                if (p.containsKey("sound")) {
                    settings = settings.sounds(blockSoundGroup(p.get("sound")));
                }
                if ("1".equals(p.get("requires_tool"))) settings = settings.requiresTool();
                if ("1".equals(p.get("no_collision")))  settings = settings.noCollision();
                if (p.containsKey("slipperiness")) {
                    settings = settings.slipperiness(parseFloat(p, "slipperiness", 0.6f));
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
                Item blockItem = new YogBlockItem(block, new Item.Settings(),
                        p.getOrDefault("name", ""));
                Registry.register(Registries.ITEM, ident, blockItem);
                tabEntries.add(blockItem);
            }
        }

        if (!tabEntries.isEmpty()) {
            ItemConvertible icon = tabEntries.get(0);
            ItemGroup group = FabricItemGroup.builder()
                    .icon(() -> new ItemStack(icon))
                    .displayName(Text.literal("Yog"))
                    .entries((displayContext, entries) -> tabEntries.forEach(entries::add))
                    .build();
            Registry.register(Registries.ITEM_GROUP, new Identifier("yog", "yog"), group);
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
