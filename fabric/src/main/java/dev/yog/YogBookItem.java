package dev.yog;

import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.util.Hand;
import net.minecraft.util.TypedActionResult;
import net.minecraft.world.World;

/** A Yog item that opens a book GUI when right-clicked. */
public class YogBookItem extends YogItem {
    private final String bookId;

    public YogBookItem(Settings settings, String displayName, String tooltip, String bookId) {
        super(settings, displayName, tooltip);
        this.bookId = bookId;
    }

    @Override
    public TypedActionResult<ItemStack> use(World world, PlayerEntity player, Hand hand) {
        if (world.isClient) {
            String json = NativeBridge.nativeBookJson(bookId);
            if (json != null && !json.equals("null")) {
                NativeBridge.openUI(bookId, true, false);
            }
        }
        return TypedActionResult.success(player.getStackInHand(hand));
    }
}