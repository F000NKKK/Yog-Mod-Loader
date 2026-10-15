package dev.yog.mixin;

import dev.yog.YogLootTableId;
import net.minecraft.loot.LootDataKey;
import net.minecraft.loot.LootManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A loot table looked up learns its id. */
@Mixin(LootManager.class)
public class LootLookupMixin {
    @Inject(method = "getElement", at = @At("RETURN"))
    private <T> void yog$stamp(LootDataKey<T> key, CallbackInfoReturnable<T> cir) {
        if (cir.getReturnValue() instanceof YogLootTableId t && t.yog$id() == null) t.yog$setId(key.id().toString());
    }
}
