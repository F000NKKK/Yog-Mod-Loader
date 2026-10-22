package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.fabricmc.fabric.api.itemgroup.v1.FabricItemGroup;
import net.fabricmc.fabric.api.networking.v1.PlayerLookup;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.fabricmc.fabric.api.screenhandler.v1.ExtendedScreenHandlerFactory;
import net.fabricmc.fabric.api.screenhandler.v1.ExtendedScreenHandlerType;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.CreativeModeTab;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Fabric's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = YogLoader.NAME;

    /** Whether this game is a client (an integrated server's too), not a dedicated server. */
    public static boolean isClient() {
        return net.fabricmc.loader.api.FabricLoader.getInstance().getEnvironmentType() == net.fabricmc.api.EnvType.CLIENT;
    }

    public static Path gameDir() {
        return FabricLoader.getInstance().getGameDir();
    }

    public static boolean isModLoaded(String id) {
        return FabricLoader.getInstance().isModLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var c : FabricLoader.getInstance().getAllMods()) {
            var m = c.getMetadata();
            String authors = m.getAuthors().stream().map(p -> p.getName()).reduce((a, b) -> a + ", " + b).orElse("");
            out.add(new YogModInfo(m.getId(), m.getName(), m.getVersion().getFriendlyString(), authors, m.getDescription()));
        }
        return out;
    }

    /** Whether {@code e} is a part of a bigger entity: Fabric has no multipart entities of its own. */
    public static boolean isPart(Entity e) {
        return false;
    }

    public static void sendToClient(ServerPlayer p, String channel, byte[] data) {
        ServerPlayNetworking.send(p, new YogPayload(channel, data));
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channel, byte[] data) {
        java.util.Set<ServerPlayer> to = new java.util.LinkedHashSet<>(PlayerLookup.tracking(e));
        if (e instanceof ServerPlayer self) to.add(self);
        for (ServerPlayer p : to) sendToClient(p, channel, data);
    }

    public static void sendToServer(String channel, byte[] data) {
        net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.send(new YogPayload(channel, data));
    }

    public static CreativeModeTab.Builder tabBuilder() {
        return FabricItemGroup.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static MenuType<YogInventoryMenu> inventoryMenuType() {
        return new ExtendedScreenHandlerType<>((id, inv, defId) -> YogInventoryMenu.createClient(id, inv, defId), ByteBufCodecs.STRING_UTF8);
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        p.openMenu(new ExtendedScreenHandlerFactory<String>() {
            @Override
            public String getScreenOpeningData(ServerPlayer player) {
                return be.defId();
            }

            @Override
            public Component getDisplayName() {
                return be.getDisplayName();
            }

            @Override
            public AbstractContainerMenu createMenu(int id, Inventory inv, Player player) {
                return be.createMenu(id, inv, player);
            }
        });
    }
}
