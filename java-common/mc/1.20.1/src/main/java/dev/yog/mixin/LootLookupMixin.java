package dev.yog.mixin;

import dev.yog.content.YogLootTableId;
import net.minecraft.world.level.storage.loot.LootDataId;
import net.minecraft.world.level.storage.loot.LootDataManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A loot table looked up learns its id. */
@Mixin(LootDataManager.class)
public class LootLookupMixin {
    @Inject(method = "getElement", at = @At("RETURN"))
    private <T> void yog$stamp(LootDataId<T> key, CallbackInfoReturnable<T> cir) {
        if (cir.getReturnValue() instanceof YogLootTableId t && t.yog$id() == null) t.yog$setId(key.location().toString());
    }
}
