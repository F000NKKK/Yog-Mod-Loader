package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.PathfinderMob;
import net.minecraft.world.entity.ai.goal.AvoidEntityGoal;
import net.minecraft.world.entity.ai.goal.FloatGoal;
import net.minecraft.world.entity.ai.goal.LookAtPlayerGoal;
import net.minecraft.world.entity.ai.goal.MeleeAttackGoal;
import net.minecraft.world.entity.ai.goal.PanicGoal;
import net.minecraft.world.entity.ai.goal.RandomLookAroundGoal;
import net.minecraft.world.entity.ai.goal.WaterAvoidingRandomStrollGoal;
import net.minecraft.world.entity.ai.goal.target.NearestAttackableTargetGoal;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;

/**
 * A mod's living thing or mob ({@code kind: living} / {@code mob}): health
 * and attributes from its {@code .yentity}, the game's AI goals it lists
 * (in priority order), a tick of its own for the mod if asked.
 */
public class YogMob extends PathfinderMob {
    public YogMob(EntityType<? extends YogMob> type, Level level) {
        super(type, level);
    }

    /** Runs in the constructor: the definition comes by the type. */
    @Override
    protected void registerGoals() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        if (def == null || !def.mob()) return;
        int priority = 0;
        for (String goal : def.goals()) {
            switch (goal) {
                case "float" -> goalSelector.addGoal(priority, new FloatGoal(this));
                case "panic" -> goalSelector.addGoal(priority, new PanicGoal(this, 1.25));
                case "melee_attack" -> goalSelector.addGoal(priority, new MeleeAttackGoal(this, 1.0, true));
                case "wander" -> goalSelector.addGoal(priority, new WaterAvoidingRandomStrollGoal(this, 1.0));
                case "look_at_player" -> goalSelector.addGoal(priority, new LookAtPlayerGoal(this, Player.class, 8.0F));
                case "look_around" -> goalSelector.addGoal(priority, new RandomLookAroundGoal(this));
                case "avoid_player" -> goalSelector.addGoal(priority, new AvoidEntityGoal<>(this, Player.class, 8.0F, 1.0, 1.2));
                case "target_player" -> targetSelector.addGoal(priority, new NearestAttackableTargetGoal<>(this, Player.class, true));
                default -> System.err.println("[yog] " + def.id() + ": no AI goal `" + goal + "`");
            }
            priority++;
        }
    }

    @Override
    public void tick() {
        super.tick();
        YogEntityTypes.tick(this);
    }

    @Override
    public boolean isPushable() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && def.pushable() != null ? def.pushable() : super.isPushable();
    }

    /** Monsters go away when no one's near, as the game's; others stay. */
    @Override
    public boolean removeWhenFarAway(double distance) {
        return getType().getCategory() == MobCategory.MONSTER;
    }

    @Override
    protected float getStandingEyeHeight(net.minecraft.world.entity.Pose pose, net.minecraft.world.entity.EntityDimensions size) {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && def.eyeHeight() != null ? def.eyeHeight() : super.getStandingEyeHeight(pose, size);
    }
}
