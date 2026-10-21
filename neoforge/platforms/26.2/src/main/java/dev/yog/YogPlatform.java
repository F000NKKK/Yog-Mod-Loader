package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.CreativeModeTab;
import net.neoforged.fml.ModList;
import net.neoforged.fml.loading.FMLPaths;
import net.neoforged.neoforge.client.network.ClientPacketDistributor;
import net.neoforged.neoforge.common.extensions.IMenuTypeExtension;
import net.neoforged.neoforge.network.PacketDistributor;
import net.neoforged.neoforge.registries.DeferredHolder;

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
        return FMLPaths.GAMEDIR.get();
    }

    public static boolean isModLoaded(String id) {
        return ModList.get().isLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var m : ModList.get().getMods()) {
            out.add(new YogModInfo(m.getModId(), m.getDisplayName(), m.getVersion().toString(), "", m.getDescription()));
        }
        return out;
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channel, byte[] data) {
        PacketDistributor.sendToPlayersTrackingEntityAndSelf(e, new YogPayload(channel, data));
    }

    public static void sendToClient(ServerPlayer p, String channel, byte[] data) {
        PacketDistributor.sendToPlayer(p, new YogPayload(channel, data));
    }

    public static void sendToServer(String channel, byte[] data) {
        ClientPacketDistributor.sendToServer(new YogPayload(channel, data));
    }

    /** A holder of {@code id} in {@code registry}: bound once it's registered (NeoForge registers registry by registry). */
    public static <T> Holder<T> holder(ResourceKey<? extends Registry<T>> registry, Identifier id) {
        return DeferredHolder.create(registry, id);
    }

    public static CreativeModeTab.Builder tabBuilder() {
        return CreativeModeTab.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static MenuType<YogInventoryMenu> inventoryMenuType() {
        return IMenuTypeExtension.create((id, inv, buf) -> YogInventoryMenu.createClient(id, inv, buf.readUtf()));
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        p.openMenu(be, buf -> buf.writeUtf(be.defId()));
    }
}
