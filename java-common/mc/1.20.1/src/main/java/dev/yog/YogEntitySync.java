package dev.yog;

import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.ClientPlayerNetworkEvent;
import net.minecraftforge.event.entity.EntityLeaveLevelEvent;
import net.minecraftforge.event.entity.player.PlayerEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

/**
 * Entity data reaches the players who see an entity (the runtime sends
 * changes, {@code sendToTracking}): a player who starts seeing an entity, or
 * comes into a world, gets all of it; an entity that leaves the server's
 * memory is forgotten (what it keeps was saved with it).
 */
@Mod.EventBusSubscriber(modid = "yog")
public final class YogEntitySync {
    private static final String CHANNEL = "yog:entity_data";

    private YogEntitySync() {}

    private static void send(ServerPlayer to, Entity about) {
        byte[] data = NativeBridge.nativeEntityDataFor(about.getStringUUID());
        if (data != null) YogNetworkBridge.sendToPlayer(to, CHANNEL, data);
    }

    @SubscribeEvent
    public static void onStartTracking(PlayerEvent.StartTracking event) {
        if (event.getEntity() instanceof ServerPlayer p) send(p, event.getTarget());
    }

    @SubscribeEvent
    public static void onLogin(PlayerEvent.PlayerLoggedInEvent event) {
        if (event.getEntity() instanceof ServerPlayer p) send(p, p);
    }

    @SubscribeEvent
    public static void onRespawn(PlayerEvent.PlayerRespawnEvent event) {
        if (event.getEntity() instanceof ServerPlayer p) send(p, p);
    }

    @SubscribeEvent
    public static void onChangedDimension(PlayerEvent.PlayerChangedDimensionEvent event) {
        if (event.getEntity() instanceof ServerPlayer p) send(p, p);
    }

    @SubscribeEvent
    public static void onLeave(EntityLeaveLevelEvent event) {
        if (event.getLevel().isClientSide() || event.getEntity() instanceof Player) return;
        NativeBridge.nativeEntityDataForget(event.getEntity().getStringUUID());
    }

    /** The client forgets a world's data when it leaves it. */
    @Mod.EventBusSubscriber(modid = "yog", value = Dist.CLIENT)
    public static final class Client {
        private Client() {}

        @SubscribeEvent
        public static void onLogout(ClientPlayerNetworkEvent.LoggingOut event) {
            NativeBridge.nativeEntityDataClientReset();
        }
    }
}
