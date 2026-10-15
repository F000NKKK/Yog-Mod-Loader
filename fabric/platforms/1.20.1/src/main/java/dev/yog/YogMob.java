package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.entity.ai.goal.ActiveTargetGoal;
import net.minecraft.entity.ai.goal.EscapeDangerGoal;
import net.minecraft.entity.ai.goal.FleeEntityGoal;
import net.minecraft.entity.ai.goal.LookAroundGoal;
import net.minecraft.entity.ai.goal.LookAtEntityGoal;
import net.minecraft.entity.ai.goal.MeleeAttackGoal;
import net.minecraft.entity.ai.goal.SwimGoal;
import net.minecraft.entity.ai.goal.WanderAroundFarGoal;
import net.minecraft.entity.mob.PathAwareEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.world.World;

/**
 * A mod's living thing or mob ({@code kind: living} / {@code mob}): health
 * and attributes from its {@code .yentity}, the game's AI goals it lists
 * (in priority order), a tick of its own for the mod if asked.
 */
public class YogMob extends PathAwareEntity {
    public YogMob(EntityType<? extends YogMob> type, World world) {
        super(type, world);
    }

    /** Runs in the constructor: the definition comes by the type. */
    @Override
    protected void initGoals() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        if (def == null || !def.mob()) return;
        int priority = 0;
        for (String goal : def.goals()) {
            switch (goal) {
                case "float" -> goalSelector.add(priority, new SwimGoal(this));
                case "panic" -> goalSelector.add(priority, new EscapeDangerGoal(this, 1.25));
                case "melee_attack" -> goalSelector.add(priority, new MeleeAttackGoal(this, 1.0, true));
                case "wander" -> goalSelector.add(priority, new WanderAroundFarGoal(this, 1.0));
                case "look_at_player" -> goalSelector.add(priority, new LookAtEntityGoal(this, PlayerEntity.class, 8.0F));
                case "look_around" -> goalSelector.add(priority, new LookAroundGoal(this));
                case "avoid_player" -> goalSelector.add(priority, new FleeEntityGoal<>(this, PlayerEntity.class, 8.0F, 1.0, 1.2));
                case "target_player" -> targetSelector.add(priority, new ActiveTargetGoal<>(this, PlayerEntity.class, true));
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
    public boolean canImmediatelyDespawn(double distance) {
        return getType().getSpawnGroup() == SpawnGroup.MONSTER;
    }

    @Override
    protected float getActiveEyeHeight(net.minecraft.entity.EntityPose pose, net.minecraft.entity.EntityDimensions size) {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && def.eyeHeight() != null ? def.eyeHeight() : super.getActiveEyeHeight(pose, size);
    }
}
