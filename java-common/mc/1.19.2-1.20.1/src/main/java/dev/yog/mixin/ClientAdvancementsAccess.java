package dev.yog.mixin;

import java.util.Map;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The client's advancement progress, for {@code YogClientApi.advancementDone}. */
@Mixin(net.minecraft.client.multiplayer.ClientAdvancements.class)
public interface ClientAdvancementsAccess {
    @Accessor("progress")
    Map<net.minecraft.advancements.Advancement, net.minecraft.advancements.AdvancementProgress> yog$progress();
}
