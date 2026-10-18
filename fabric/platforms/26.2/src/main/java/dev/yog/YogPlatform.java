package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.fabricmc.fabric.api.networking.v1.PlayerLookup;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Fabric's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = "fabric";

    /** The game's directory. */
    public static Path gameDir() {
        return FabricLoader.getInstance().getGameDir();
    }

    /** Whether Java mod {@code id} is loaded. */
    public static boolean isModLoaded(String id) {
        return FabricLoader.getInstance().isModLoaded(id);
    }

    /** Every Java mod loaded. */
    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var c : FabricLoader.getInstance().getAllMods()) {
            var m = c.getMetadata();
            String authors = m.getAuthors().stream().map(p -> p.getName()).reduce((a, b) -> a + ", " + b).orElse("");
            out.add(new YogModInfo(m.getId(), m.getName(), m.getVersion().getFriendlyString(), authors, m.getDescription()));
        }
        return out;
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channel, byte[] data) {
        java.util.Set<ServerPlayer> to = new java.util.LinkedHashSet<>(PlayerLookup.tracking(e));
        if (e instanceof ServerPlayer self) to.add(self);
        for (ServerPlayer p : to) sendToClient(p, channel, data);
    }

    /** Raw bytes to {@code p} on {@code channel}. */
    public static void sendToClient(ServerPlayer p, String channel, byte[] data) {
        ServerPlayNetworking.send(p, new YogPayload(channel, data));
    }

    /** A holder of {@code id} in {@code registry}: registered by now (Fabric registers at once). */
    public static <T> net.minecraft.core.Holder<T> holder(net.minecraft.resources.ResourceKey<? extends net.minecraft.core.Registry<T>> registry, net.minecraft.resources.Identifier id) {
        @SuppressWarnings("unchecked")
        net.minecraft.core.Registry<T> r = (net.minecraft.core.Registry<T>) net.minecraft.core.registries.BuiltInRegistries.REGISTRY.getValue(registry.identifier());
        return r.getOrThrow(net.minecraft.resources.ResourceKey.create(registry, id));
    }

    /** A creative tab's builder. */
    public static net.minecraft.world.item.CreativeModeTab.Builder tabBuilder() {
        return net.fabricmc.fabric.api.creativetab.v1.FabricCreativeModeTab.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static net.minecraft.world.inventory.MenuType<YogInventoryMenu> inventoryMenuType() {
        return new net.fabricmc.fabric.api.menu.v1.ExtendedMenuType<>(YogInventoryMenu::createClient, net.minecraft.network.codec.ByteBufCodecs.STRING_UTF8);
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        p.openMenu(new net.fabricmc.fabric.api.menu.v1.ExtendedMenuProvider<String>() {
            @Override
            public String getScreenOpeningData(ServerPlayer player) {
                return be.defId();
            }

            @Override
            public net.minecraft.network.chat.Component getDisplayName() {
                return be.getDisplayName();
            }

            @Override
            public net.minecraft.world.inventory.AbstractContainerMenu createMenu(int id, net.minecraft.world.entity.player.Inventory inv, net.minecraft.world.entity.player.Player player) {
                return be.createMenu(id, inv, player);
            }
        });
    }

    /** Raw bytes to the server on {@code channel} (client side). */
    public static void sendToServer(String channel, byte[] data) {
        net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.send(new YogPayload(channel, data));
    }
}
