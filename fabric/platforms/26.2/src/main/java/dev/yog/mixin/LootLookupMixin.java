package dev.yog.mixin;

import dev.yog.YogLootTableId;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.ReloadableServerRegistries;
import net.minecraft.world.level.storage.loot.LootTable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A loot table looked up learns its id. */
@Mixin(ReloadableServerRegistries.Holder.class)
public class LootLookupMixin {
    @Inject(method = "getLootTable", at = @At("RETURN"))
    private void yog$stamp(ResourceKey<LootTable> key, CallbackInfoReturnable<LootTable> cir) {
        if (cir.getReturnValue() instanceof YogLootTableId t && t.yog$id() == null) t.yog$setId(key.identifier().toString());
    }
}
