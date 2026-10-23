package dev.yog.client;

import dev.yog.NativeBridge;
import dev.yog.render.NativeDraw;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent;

/** Draws a {@link YogTooltipData}: the mod's code paints it through the runtime. */
public record YogClientTooltip(YogTooltipData data) implements ClientTooltipComponent {
    /** Space under the picture before the next line. */
    private static final int GAP = 2;

    @Override
    public int getHeight(Font font) {
        return data.height() + GAP;
    }

    @Override
    public int getWidth(Font font) {
        return data.width();
    }

    @Override
    public void extractImage(Font font, int x, int y, int w, int h, GuiGraphicsExtractor ctx) {
        var window = Minecraft.getInstance().getWindow();
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeTooltipDraw(data.item(), data.snbt(), x, y, data.width(), data.height(),
                window.getGuiScaledWidth(), window.getGuiScaledHeight());
        NativeDraw.hudDrawContext = null;
    }
}
