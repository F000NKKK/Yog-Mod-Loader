package dev.yog;

import net.minecraft.world.entity.EquipmentSlot;

/**
 * A Yog item worn in an armor slot without being armor ({@code item.wearable}):
 * its equippable component lets right-click and dispensers put it on.
 */
public class YogWearableItem extends YogItem {
    public YogWearableItem(Properties settings, String displayName, String tooltip, String slot) {
        super(settings.equippable(slotOf(slot)), displayName, tooltip);
    }

    /** {@code head}, {@code chest}, {@code legs}, {@code feet} as the game's slot. */
    static EquipmentSlot slotOf(String slot) {
        return switch (slot) {
            case YogNames.ArmorSlot.CHEST -> EquipmentSlot.CHEST;
            case YogNames.ArmorSlot.LEGS -> EquipmentSlot.LEGS;
            case YogNames.ArmorSlot.FEET -> EquipmentSlot.FEET;
            default -> EquipmentSlot.HEAD;
        };
    }
}
