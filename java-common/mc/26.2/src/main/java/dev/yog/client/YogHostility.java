package dev.yog.client;

import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.NeutralMob;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.monster.spider.Spider;
import net.minecraft.world.entity.monster.piglin.AbstractPiglin;
import net.minecraft.world.entity.monster.piglin.PiglinBrute;

/**
 * Whether an entity is hostile to the player now, as the client can tell:
 * a mob attacking ({@link Mob#isAggressive()}, synced to clients), or a
 * monster that never waits to be provoked. A neutral mob — an enderman, a
 * zombified piglin, a piglin (gold worn), a spider by day — is hostile only
 * while it attacks; animals and golems never are.
 */
public final class YogHostility {
    private YogHostility() {}

    public static boolean hostileNow(Entity e) {
        if (e instanceof Mob m && m.isAggressive()) return true;
        return e instanceof Enemy && !neutral(e);
    }

    /** A monster that attacks only when provoked or by its own rule. */
    private static boolean neutral(Entity e) {
        return e instanceof NeutralMob
                || e instanceof Spider
                || e instanceof AbstractPiglin && !(e instanceof PiglinBrute);
    }
}
