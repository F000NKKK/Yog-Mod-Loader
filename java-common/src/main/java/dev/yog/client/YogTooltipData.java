package dev.yog.client;

import dev.yog.NativeBridge;

import dev.yog.bus.YogServices;
import java.util.Optional;

/**
 * A Yog mod's picture under an item's tooltip ({@code Registry::on_item_tooltip},
 * the {@code item/picture} service): the stack's item and custom data, and the picture's size.
 */
public record YogTooltipData(String item, String snbt, int width, int height) implements net.minecraft.world.inventory.tooltip.TooltipComponent {
    /** The picture a mod gives {@code stack}, if any. */
    public static Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> of(net.minecraft.world.item.ItemStack stack) {
        String item = stack.getItem().builtInRegistryHolder().key().location().toString();
        String snbt = NativeBridge.tooltipSnbt(stack);
        float[] size;
        try {
            size = new YogServices.Item.Picture(item, snbt).call();
        } catch (UnsatisfiedLinkError e) {
            return Optional.empty();
        }
        if (size == null || size.length < 2 || size[0] <= 0 || size[1] <= 0) return Optional.empty();
        return Optional.of(new YogTooltipData(item, snbt, Math.round(size[0]), Math.round(size[1])));
    }
}
