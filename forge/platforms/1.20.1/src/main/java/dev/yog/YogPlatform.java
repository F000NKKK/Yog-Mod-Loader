package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Forge's answers (NeoForge 1.20.1's too). Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = YogLoader.NAME;

    public static Path gameDir() {
        return net.minecraftforge.fml.loading.FMLPaths.GAMEDIR.get();
    }

    public static boolean isModLoaded(String id) {
        return net.minecraftforge.fml.ModList.get() != null && net.minecraftforge.fml.ModList.get().isLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var m : net.minecraftforge.fml.ModList.get().getMods()) {
            out.add(new YogModInfo(m.getModId(), m.getDisplayName(), m.getVersion().toString(), "", m.getDescription()));
        }
        return out;
    }

    /** Whether {@code e} is a part of a bigger entity (the loader's multipart entities). */
    public static boolean isPart(Entity e) {
        return e instanceof net.minecraftforge.entity.PartEntity<?>;
    }

    /** The version of Yog's network protocol (its one channel). */
    private static final String PROTOCOL = "1";
    /** The id of Yog's one kind of message. */
    private static final int MESSAGE = 0;
    /** Yog's one channel: every mod channel's messages, as {@link YogPayload}s. */
    private static net.minecraftforge.network.simple.SimpleChannel channel;

    /** Opens Yog's channel; what comes on it goes to the mods. */
    static void openChannel() {
        channel = net.minecraftforge.network.NetworkRegistry.newSimpleChannel(YogPayload.CHANNEL, () -> PROTOCOL, PROTOCOL::equals, PROTOCOL::equals);
        channel.registerMessage(MESSAGE, YogPayload.class, YogPayload::write, YogPayload::read, (payload, context) -> {
            var ctx = context.get();
            ctx.setPacketHandled(true);
            ServerPlayer sender = ctx.getSender();
            if (sender != null) ctx.enqueueWork(() -> payload.toServer(sender));
            else ctx.enqueueWork(payload::toClient);
        });
    }

    public static void sendToClient(ServerPlayer p, String channelName, byte[] data) {
        channel.send(net.minecraftforge.network.PacketDistributor.PLAYER.with(() -> p), new YogPayload(channelName, data));
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channelName, byte[] data) {
        channel.send(net.minecraftforge.network.PacketDistributor.TRACKING_ENTITY_AND_SELF.with(() -> e), new YogPayload(channelName, data));
    }

    public static void sendToServer(String channelName, byte[] data) {
        channel.sendToServer(new YogPayload(channelName, data));
    }

    public static net.minecraft.world.item.CreativeModeTab.Builder tabBuilder() {
        return net.minecraft.world.item.CreativeModeTab.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static net.minecraft.world.inventory.MenuType<YogInventoryMenu> inventoryMenuType() {
        return net.minecraftforge.common.extensions.IForgeMenuType.create(YogInventoryMenu::createClient);
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        net.minecraftforge.network.NetworkHooks.openScreen(p, be, buf -> buf.writeUtf(be.defId()));
    }
}
