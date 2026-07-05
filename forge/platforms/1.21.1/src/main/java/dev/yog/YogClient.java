package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.resources.ResourceLocation;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;
import org.joml.Matrix4f;

/**
 * Forge 1.21.1 client-side handlers.
 * The Forge compat layer does not include NeoForge client rendering events;
 * client-world rendering and packet I/O are ported where possible.
 */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.FORGE, value = Dist.CLIENT)
public final class YogClient {
    private YogClient() {}

    // Client tick is handled by a mixin (KeyboardMixin) or removed.
    // HUD / world render events are not available in the Forge 1.21 compat layer.

    /** Send a raw-byte packet to the server (client -> server) via YogPayload. */
    public static boolean sendToServer(String channel, byte[] data) {
        ResourceLocation id = ResourceLocation.tryParse(channel);
        if (id == null) return false;
        try {
            var conn = Minecraft.getInstance().getConnection();
            if (conn == null) return false;
            conn.send(new net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(
                    new YogPayload(YogPayload.typeFor(id), data)));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
