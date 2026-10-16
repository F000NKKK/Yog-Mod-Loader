package dev.yog.mixin;

import java.util.Map;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The client's advancement progress, for {@code YogClientApi.advancementDone}. */
@Mixin(net.minecraft.client.network.ClientAdvancementManager.class)
public interface ClientAdvancementsAccess {
    @Accessor("advancementProgresses")
    Map<net.minecraft.advancement.AdvancementEntry, net.minecraft.advancement.AdvancementProgress> yog$progress();
}
