package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent;

/** Draws a {@link YogTooltipData}: the mod's code paints it through the runtime. */
public record YogClientTooltip(YogTooltipData data) implements ClientTooltipComponent {
    @Override
    public int getHeight() {
        return data.height() + 2;
    }

    @Override
    public int getWidth(Font font) {
        return data.width();
    }

    @Override
    public void renderImage(Font font, int x, int y, GuiGraphics ctx) {
        ctx.flush();
        var window = Minecraft.getInstance().getWindow();
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeTooltipDraw(data.item(), data.snbt(), x, y, data.width(), data.height(),
                window.getGuiScaledWidth(), window.getGuiScaledHeight());
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
    }
}
