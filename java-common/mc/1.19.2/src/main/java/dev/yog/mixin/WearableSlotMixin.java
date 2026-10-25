package dev.yog.mixin;

import dev.yog.content.items.YogWearableItem;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A mod's wearable goes to its own slot (dispensers, mobs): this version asks only armor for one. */
@Mixin(Mob.class)
public class WearableSlotMixin {
    @Inject(method = "getEquipmentSlotForItem", at = @At("HEAD"), cancellable = true)
    private static void yog$wearable(ItemStack stack, CallbackInfoReturnable<EquipmentSlot> cir) {
        if (stack.getItem() instanceof YogWearableItem w) cir.setReturnValue(w.slot());
    }
}
