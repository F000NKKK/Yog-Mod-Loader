package dev.yog;

import java.util.Optional;

/**
 * A Yog mod's picture under an item's tooltip ({@code Registry::on_item_tooltip},
 * ABI 2.2): the stack's item and custom data, and the picture's size.
 */
public record YogTooltipData(String item, String snbt, int width, int height) implements net.minecraft.world.inventory.tooltip.TooltipComponent {
    /** The picture a mod gives {@code stack}, if any. */
    public static Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> of(net.minecraft.world.item.ItemStack stack) {
        String item = net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        String snbt = NativeBridge.tooltipSnbt(stack);
        float[] size;
        try {
            size = NativeBridge.nativeTooltipSize(item, snbt);
        } catch (UnsatisfiedLinkError e) {
            return Optional.empty();
        }
        if (size == null || size.length < 2) return Optional.empty();
        return Optional.of(new YogTooltipData(item, snbt, Math.round(size[0]), Math.round(size[1])));
    }
}
