package dev.yog.content.items;

import dev.yog.bus.YogTypes;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResultHolder;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.player.Player;
import net.minecraft.stats.Stats;
import net.minecraft.world.item.ArmorItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.DispenserBlock;

/** A Yog item worn in an armor slot without being armor ({@code item.wearable}): right-click and dispensers put it on. */
public class YogWearableItem extends YogItem {
    private final EquipmentSlot slot;

    public YogWearableItem(Properties properties, String displayName, String tooltip, String slot) {
        super(properties, displayName, tooltip);
        this.slot = switch (slot) {
            case YogTypes.ArmorSlot.CHEST -> EquipmentSlot.CHEST;
            case YogTypes.ArmorSlot.LEGS -> EquipmentSlot.LEGS;
            case YogTypes.ArmorSlot.FEET -> EquipmentSlot.FEET;
            default -> EquipmentSlot.HEAD;
        };
        DispenserBlock.registerBehavior(this, ArmorItem.DISPENSE_ITEM_BEHAVIOR);
    }

    /** The slot it's worn in (what {@code WearableSlotMixin} tells the game). */
    public EquipmentSlot slot() {
        return slot;
    }

    /** Put on when that slot is empty, as armor is. */
    @Override
    public InteractionResultHolder<ItemStack> use(Level level, Player player, InteractionHand hand) {
        ItemStack held = player.getItemInHand(hand);
        if (!player.getItemBySlot(slot).isEmpty()) return InteractionResultHolder.fail(held);
        player.setItemSlot(slot, held.copy());
        if (!level.isClientSide()) player.awardStat(Stats.ITEM_USED.get(this));
        held.setCount(0);
        return InteractionResultHolder.sidedSuccess(held, level.isClientSide());
    }
}
