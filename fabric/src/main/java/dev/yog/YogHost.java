package dev.yog;

import com.mojang.brigadier.Command;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import net.fabricmc.api.ModInitializer;
import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseItemCallback;
import net.fabricmc.fabric.api.message.v1.ServerMessageEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.command.ServerCommandSource;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.text.Text;
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

    /** Register custom items/blocks declared by Rust mods. */
    private static void registerContent() {
        String items = NativeBridge.nativeItemDefs();
        if (items != null) {
            for (String line : items.split("\n")) {
                if (line.isBlank()) {
                    continue;
                }
                String[] p = line.split("\t");
                Identifier id = Identifier.tryParse(p[0]);
                if (id == null) {
                    continue;
                }
                int maxStack = p.length > 1 ? Integer.parseInt(p[1]) : 64;
                Registry.register(Registries.ITEM, id, new Item(new Item.Settings().maxCount(maxStack)));
            }
        }

        String blocks = NativeBridge.nativeBlockDefs();
        if (blocks != null) {
            for (String line : blocks.split("\n")) {
                if (line.isBlank()) {
                    continue;
                }
                String[] p = line.split("\t");
                Identifier id = Identifier.tryParse(p[0]);
                if (id == null) {
                    continue;
                }
                float hardness = p.length > 1 ? Float.parseFloat(p[1]) : 1.5f;
                float resistance = p.length > 2 ? Float.parseFloat(p[2]) : 6.0f;
                Block block = new Block(AbstractBlock.Settings.create().strength(hardness, resistance));
                Registry.register(Registries.BLOCK, id, block);
                Registry.register(Registries.ITEM, id, new BlockItem(block, new Item.Settings()));
            }
        }
    }

    private static int runCommand(String name, String args, CommandContext<ServerCommandSource> ctx) {
        ServerCommandSource src = ctx.getSource();
        String reply = NativeBridge.nativeOnCommand(name, args, src.getName());
        if (reply != null && !reply.isEmpty()) {
            src.sendFeedback(() -> Text.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }
}
