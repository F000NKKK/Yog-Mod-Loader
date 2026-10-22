package dev.yog;

import dev.yog.util.YogModInfo;

import io.netty.buffer.Unpooled;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraftforge.common.extensions.IForgeMenuType;
import net.minecraftforge.fml.ModList;
import net.minecraftforge.fml.loading.FMLPaths;
import net.minecraftforge.network.ChannelBuilder;
import net.minecraftforge.network.EventNetworkChannel;
import net.minecraftforge.network.PacketDistributor;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Forge's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = YogLoader.NAME;
    /** The version of Yog's network protocol (its one channel). */
    private static final int PROTOCOL = 1;

    /** Yog's one channel: every mod channel's messages, as {@link YogPayload}s. */
    private static EventNetworkChannel channel;

    /** Opens Yog's channel; what comes on it goes to the mods. */
    static void openChannel() {
        channel = ChannelBuilder.named(YogPayload.TYPE.id()).networkProtocolVersion(PROTOCOL).optional().eventNetworkChannel();
        channel.addListener(event -> {
            FriendlyByteBuf buf = event.getPayload();
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

    private static FriendlyByteBuf frame(String channelName, byte[] data) {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        YogPayload.CODEC.encode(buf, new YogPayload(channelName, data));
        return buf;
    }

    /** Whether this game is a client (an integrated server's too), not a dedicated server. */
    public static boolean isClient() {
        return net.minecraftforge.fml.loading.FMLEnvironment.dist == net.minecraftforge.api.distmarker.Dist.CLIENT;
    }

    public static Path gameDir() {
        return FMLPaths.GAMEDIR.get();
    }

    public static boolean isModLoaded(String id) {
        return ModList.isLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var m : ModList.getMods()) {
            out.add(new YogModInfo(m.getModId(), m.getDisplayName(), m.getVersion().toString(), "", m.getDescription()));
        }
        return out;
    }

    public static void sendToClient(ServerPlayer p, String channelName, byte[] data) {
        channel.send(frame(channelName, data), PacketDistributor.PLAYER.with(p));
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channelName, byte[] data) {
        channel.send(frame(channelName, data), PacketDistributor.TRACKING_ENTITY_AND_SELF.with(e));
    }

    public static void sendToServer(String channelName, byte[] data) {
        channel.send(frame(channelName, data), PacketDistributor.SERVER.noArg());
    }

    /** A holder of {@code id} in {@code registry}: Forge's register events put a thing in at once. */
    public static <T> Holder<T> holder(ResourceKey<? extends Registry<T>> registry, Identifier id) {
        @SuppressWarnings("unchecked") // a registry of the key's type: the root registry hands it out untyped
        Registry<T> r = (Registry<T>) BuiltInRegistries.REGISTRY.getValue(registry.identifier());
        return r.getOrThrow(ResourceKey.create(registry, id));
    }

    public static CreativeModeTab.Builder tabBuilder() {
        return CreativeModeTab.builder();
    }

    /** The menu type of the mods' inventories: the inventory's id comes with the screen. */
    public static MenuType<YogInventoryMenu> inventoryMenuType() {
        return IForgeMenuType.create((id, inv, buf) -> YogInventoryMenu.createClient(id, inv, buf.readUtf()));
    }

    /** Opens {@code be}'s inventory for {@code p}, its id sent along. */
    public static void openInventory(ServerPlayer p, YogInventoryBlockEntity be) {
        p.openMenu(be, buf -> buf.writeUtf(be.defId()));
    }
}
