package dev.yog;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.level.block.Block;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.RegisterColorHandlersEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

/** Tints a mod sets per position ({@code client::set_block_tint}), on faces with a tint index. */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.MOD, value = Dist.CLIENT)
public final class YogColors {
    private YogColors() {}

    @SubscribeEvent
    public static void onBlockColors(RegisterColorHandlersEvent.Block event) {
        List<Block> blocks = new ArrayList<>();
        for (dev.yog.data.YogContent.Block b : dev.yog.data.YogContent.cached(NativeBridge::nativeContent).blocks) {
            ResourceLocation id = ResourceLocation.tryParse(b.id());
            if (id != null && BuiltInRegistries.BLOCK.containsKey(id)) blocks.add(BuiltInRegistries.BLOCK.get(id));
        }
        if (!blocks.isEmpty()) {
            event.register((state, level, pos, tintIndex) -> YogClientApi.tint(pos), blocks.toArray(new Block[0]));
        }
    }
}
