package dev.yog.mixin;

import dev.yog.content.YogLootHook;
import dev.yog.content.YogLootTableId;
import java.util.function.Consumer;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.storage.loot.LootContext;
import net.minecraft.world.level.storage.loot.LootTable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Loot tables rolled at the top go past the mods ({@link YogLootHook}); a
 * table knows its id once it's been looked up ({@code LootLookupMixin}).
 */
@Mixin(LootTable.class)
public class LootTableMixin implements YogLootTableId {
    @Unique
    private String yog$id;

    @Override
    public String yog$id() {
        return yog$id;
    }

    @Override
    public void yog$setId(String id) {
        yog$id = id;
    }

    @Inject(method = "getRandomItemsRaw(Lnet/minecraft/world/level/storage/loot/LootContext;Ljava/util/function/Consumer;)V", at = @At("HEAD"), cancellable = true)
    private void yog$loot(LootContext ctx, Consumer<ItemStack> out, CallbackInfo ci) {
        if (YogLootHook.roll((LootTable) (Object) this, yog$id, ctx, out)) ci.cancel();
    }
}
