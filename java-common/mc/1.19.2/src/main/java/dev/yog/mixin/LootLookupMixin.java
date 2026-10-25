package dev.yog.mixin;

import dev.yog.content.YogLootTableId;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.LootTables;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A loot table looked up learns its id (not the shared empty one a missing id gets). */
@Mixin(LootTables.class)
public class LootLookupMixin {
    @Inject(method = "get", at = @At("RETURN"))
    private void yog$stamp(ResourceLocation id, CallbackInfoReturnable<LootTable> cir) {
        LootTable table = cir.getReturnValue();
        if (table != LootTable.EMPTY && table instanceof YogLootTableId t && t.yog$id() == null) t.yog$setId(id.toString());
    }
}
