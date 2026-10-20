package dev.yog;

import dev.yog.bus.YogServices;
import java.util.Optional;

/**
 * A Yog mod's picture under an item's tooltip ({@code Registry::on_item_tooltip},
 * the {@code item/picture} service): the stack's item and custom data, and the picture's size.
 */
public record YogTooltipData(String item, String snbt, int width, int height) implements net.minecraft.world.inventory.tooltip.TooltipComponent {
    /** The picture a mod gives {@code stack}, if any. */
    public static Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> of(net.minecraft.world.item.ItemStack stack) {
        String item = net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        String snbt = NativeBridge.tooltipSnbt(stack);
        java.util.List<Float> size;
        try {
            size = new YogServices.Item.Picture(item, snbt).call();
        } catch (UnsatisfiedLinkError e) {
            return Optional.empty();
        }
        if (size == null || size.size() < 2 || size.get(0) <= 0 || size.get(1) <= 0) return Optional.empty();
        return Optional.of(new YogTooltipData(item, snbt, (int) Math.round(size.get(0)), (int) Math.round(size.get(1))));
    }
}
