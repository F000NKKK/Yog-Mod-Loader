package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: NeoForge's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = YogLoader.NAME;

    public static Path gameDir() {
        return net.neoforged.fml.loading.FMLPaths.GAMEDIR.get();
    }

    public static boolean isModLoaded(String id) {
        return net.neoforged.fml.ModList.get() != null && net.neoforged.fml.ModList.get().isLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var m : net.neoforged.fml.ModList.get().getMods()) {
            out.add(new YogModInfo(m.getModId(), m.getDisplayName(), m.getVersion().toString(), "", m.getDescription()));
        }
        return out;
    }

    /** Whether {@code e} is a part of a bigger entity (the loader's multipart entities). */
    public static boolean isPart(Entity e) {
        return e instanceof net.neoforged.neoforge.entity.PartEntity<?>;
    }

    public static void sendToClient(ServerPlayer p, String channel, byte[] data) {
        net.neoforged.neoforge.network.PacketDistributor.sendToPlayer(p, new YogPayload(channel, data));
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channel, byte[] data) {
        net.neoforged.neoforge.network.PacketDistributor.sendToPlayersTrackingEntityAndSelf(e, new YogPayload(channel, data));
    }

    public static void sendToServer(String channel, byte[] data) {
        net.neoforged.neoforge.network.PacketDistributor.sendToServer(new YogPayload(channel, data));
    }

    public static net.minecraft.world.item.CreativeModeTab.Builder tabBuilder() {
        return net.minecraft.world.item.CreativeModeTab.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static net.minecraft.world.inventory.MenuType<YogInventoryMenu> inventoryMenuType() {
        return net.neoforged.neoforge.common.extensions.IMenuTypeExtension.create(YogInventoryMenu::createClient);
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        p.openMenu(be, buf -> buf.writeUtf(be.defId()));
    }
}
