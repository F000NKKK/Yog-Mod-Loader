package dev.yog.server;

import dev.yog.YogServiceHost;

import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * How fast this client's player digs a block with the best tool in its
 * hotbar — Baritone's {@code ToolSet} (swords aren't used to dig; among
 * equals, the cheaper material), its haste and mining fatigue counted in.
 */
public final class YogDigSpeeds {
    private YogDigSpeeds() {}

    /** The hotbar's slots. */
    private static final int HOTBAR = 9;
    /** A block broken in this many ticks' worth of speed: the right tool's, the wrong one's. */
    private static final float RIGHT_TOOL = 30f, WRONG_TOOL = 100f;
    /** Haste's speed-up a level. */
    private static final double HASTE_STEP = 0.2;
    /** Mining fatigue's slow-down by level (past the last, the last). */
    private static final double[] FATIGUE = {0.3, 0.09, 0.0027, 0.00081};

    @YogServiceHost
    public static List<YogTypes.DigSpeed> of(List<String> blocks) {
        LocalPlayer p = Minecraft.getInstance().player;
        List<YogTypes.DigSpeed> out = new ArrayList<>(blocks.size());
        double effects = p == null ? 1 : effects(p);
        for (String id : blocks) {
            Block b = p == null ? null : YogDigging.block(id);
            if (b == null) {
                out.add(new YogTypes.DigSpeed(0, 0f));
                continue;
            }
            BlockState st = b.defaultBlockState();
            int slot = best(p, st);
            double speed = speed(p, p.getInventory().getItem(slot), st);
            out.add(new YogTypes.DigSpeed(slot, (float) Math.max(0, speed * effects)));
        }
        return out;
    }

    /** The hotbar slot digging {@code st} fastest (`getBestSlot`). */
    private static int best(LocalPlayer p, BlockState st) {
        int best = 0;
        double fastest = Double.NEGATIVE_INFINITY;
        int cheapest = Integer.MIN_VALUE;
        for (int i = 0; i < HOTBAR; i++) {
            ItemStack stack = p.getInventory().getItem(i);
            if (YogDigging.sword(stack)) continue;
            double speed = speed(p, stack, st);
            int cost = YogDigging.material(stack);
            if (speed > fastest || (speed == fastest && cost < cheapest)) {
                fastest = speed;
                best = i;
                cheapest = cost;
            }
        }
        return best;
    }

    /** The share of {@code st} a tick of digging with {@code item} breaks; negative: it can't be (`calculateSpeedVsBlock`). */
    private static double speed(LocalPlayer p, ItemStack item, BlockState st) {
        float hardness = st.getDestroySpeed(p.level(), p.blockPosition());
        if (hardness < 0) return -1;
        float speed = item.getDestroySpeed(st);
        if (speed > 1) {
            int level = YogDigging.efficiency(p, item);
            if (level > 0) speed += level * level + 1;
        }
        speed /= hardness;
        boolean right = !st.requiresCorrectToolForDrops() || (!item.isEmpty() && item.isCorrectToolForDrops(st));
        return speed / (right ? RIGHT_TOOL : WRONG_TOOL);
    }

    /** Haste and mining fatigue's change to digging (`potionAmplifier`). */
    private static double effects(LocalPlayer p) {
        double speed = 1;
        int haste = YogDigging.haste(p);
        if (haste >= 0) speed *= 1 + (haste + 1) * HASTE_STEP;
        int fatigue = YogDigging.fatigue(p);
        if (fatigue >= 0) speed *= FATIGUE[Math.min(fatigue, FATIGUE.length - 1)];
        return speed;
    }
}
