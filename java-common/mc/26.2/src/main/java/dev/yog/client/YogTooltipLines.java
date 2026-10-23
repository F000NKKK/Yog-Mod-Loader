package dev.yog.client;

import java.util.function.Consumer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.ItemStack;

/**
 * An item whose tooltip has lines from a Yog mod; the client adds them
 * (Fabric's tooltip event, as the item's own hook is deprecated in 26.x).
 */
public interface YogTooltipLines {
    void tooltipLines(ItemStack stack, Consumer<Component> lines);
}
