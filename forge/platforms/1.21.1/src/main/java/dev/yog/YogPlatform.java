package dev.yog;

import dev.yog.content.blocks.YogInventoryBlockEntity;
import dev.yog.content.blocks.YogInventoryMenu;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Forge's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = YogLoader.NAME;

    /** Whether this game is a client (an integrated server's too), not a dedicated server. */
    public static boolean isClient() {
        return net.minecraftforge.fml.loading.FMLEnvironment.dist == net.minecraftforge.api.distmarker.Dist.CLIENT;
    }

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
    private static final int PROTOCOL = 1;
    /** Yog's one channel: every mod channel's messages, as {@link YogPayload}s. */
    private static net.minecraftforge.network.EventNetworkChannel channel;

    /** Opens Yog's channel; what comes on it goes to the mods. */
    static void openChannel() {
        channel = net.minecraftforge.network.ChannelBuilder.named(YogPayload.TYPE.id()).networkProtocolVersion(PROTOCOL).optional().eventNetworkChannel();
        channel.addListener(event -> {
            net.minecraft.network.FriendlyByteBuf buf = event.getPayload();
            if (buf == null) return;
            YogPayload payload = YogPayload.CODEC.decode(buf);
            var ctx = event.getSource();
            ctx.setPacketHandled(true);
            if (ctx.isServerSide()) {
                ServerPlayer sp = ctx.getSender();
                if (sp != null) ctx.enqueueWork(() -> payload.toServer(sp));
            } else {
                ctx.enqueueWork(payload::toClient);
            }
        });
    }

    private static net.minecraft.network.FriendlyByteBuf frame(String channelName, byte[] data) {
        var buf = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
        YogPayload.CODEC.encode(buf, new YogPayload(channelName, data));
        return buf;
    }

    public static void sendToClient(ServerPlayer p, String channelName, byte[] data) {
        channel.send(frame(channelName, data), net.minecraftforge.network.PacketDistributor.PLAYER.with(p));
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channelName, byte[] data) {
        channel.send(frame(channelName, data), net.minecraftforge.network.PacketDistributor.TRACKING_ENTITY_AND_SELF.with(e));
    }

    public static void sendToServer(String channelName, byte[] data) {
        // A server without Yog has no such channel: nothing to send it.
        var conn = net.minecraft.client.Minecraft.getInstance().getConnection();
        if (conn == null || !channel.isRemotePresent(conn.getConnection())) return;
        channel.send(frame(channelName, data), net.minecraftforge.network.PacketDistributor.SERVER.noArg());
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
        p.openMenu(be, buf -> buf.writeUtf(be.defId()));
    }
}
