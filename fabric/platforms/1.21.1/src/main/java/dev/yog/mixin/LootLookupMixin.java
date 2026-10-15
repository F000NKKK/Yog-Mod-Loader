package dev.yog.mixin;

import dev.yog.YogLootTableId;
import net.minecraft.loot.LootTable;
import net.minecraft.registry.RegistryKey;
import net.minecraft.registry.ReloadableRegistries;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A loot table looked up learns its id. */
@Mixin(ReloadableRegistries.Lookup.class)
public class LootLookupMixin {
    @Inject(method = "getLootTable", at = @At("RETURN"))
    private void yog$stamp(RegistryKey<LootTable> key, CallbackInfoReturnable<LootTable> cir) {
        if (cir.getReturnValue() instanceof YogLootTableId t && t.yog$id() == null) t.yog$setId(key.getValue().toString());
    }
}
