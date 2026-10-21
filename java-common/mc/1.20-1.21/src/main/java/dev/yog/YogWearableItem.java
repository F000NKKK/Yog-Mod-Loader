package dev.yog;

import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResultHolder;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ArmorItem;
import net.minecraft.world.item.Equipable;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.DispenserBlock;

/** A Yog item worn in an armor slot without being armor ({@code item.wearable}): right-click and dispensers put it on. */
public class YogWearableItem extends YogItem implements Equipable {
    private final EquipmentSlot slot;

    public YogWearableItem(Properties properties, String displayName, String tooltip, String slot) {
        super(properties, displayName, tooltip);
        this.slot = switch (slot) {
            case YogNames.ArmorSlot.CHEST -> EquipmentSlot.CHEST;
            case YogNames.ArmorSlot.LEGS -> EquipmentSlot.LEGS;
            case YogNames.ArmorSlot.FEET -> EquipmentSlot.FEET;
            default -> EquipmentSlot.HEAD;
        };
        DispenserBlock.registerBehavior(this, ArmorItem.DISPENSE_ITEM_BEHAVIOR);
    }

    @Override
    public EquipmentSlot getEquipmentSlot() {
        return slot;
    }

    @Override
    public InteractionResultHolder<ItemStack> use(Level level, Player player, InteractionHand hand) {
        return swapWithEquipmentSlot(this, level, player, hand);
    }
}
