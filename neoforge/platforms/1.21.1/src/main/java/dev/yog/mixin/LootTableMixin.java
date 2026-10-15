package dev.yog.mixin;

import dev.yog.YogLootHook;
import java.util.function.Consumer;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.storage.loot.LootContext;
import net.minecraft.world.level.storage.loot.LootTable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Loot tables rolled at the top go past the mods ({@link YogLootHook}). */
@Mixin(LootTable.class)
public class LootTableMixin {
    @Inject(method = "getRandomItems(Lnet/minecraft/world/level/storage/loot/LootContext;Ljava/util/function/Consumer;)V", at = @At("HEAD"), cancellable = true)
    private void yog$loot(LootContext ctx, Consumer<ItemStack> out, CallbackInfo ci) {
        LootTable self = (LootTable) (Object) this;
        var id = self.getLootTableId();
        if (YogLootHook.roll(self, id == null ? null : id.toString(), ctx, out)) ci.cancel();
    }
}
