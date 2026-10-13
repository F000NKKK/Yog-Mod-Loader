package dev.yog.util;

/**
 * Entity kind bits in the {@code entityInfo} reply — mirror of
 * {@code yog_core::query::entity_flags} (the Rust side decodes them).
 */
public final class YogEntityFlags {
    private YogEntityFlags() {
    }

    /** A {@code LivingEntity} (mobs, players, armor stands). */
    public static final int LIVING = 1;
    /** A player. */
    public static final int PLAYER = 1 << 1;
    /** An {@code Animal} or {@code WaterAnimal}. */
    public static final int ANIMAL = 1 << 2;
    /** An {@code Enemy} (hostile mobs). */
    public static final int MONSTER = 1 << 3;
    /** A dropped item ({@code ItemEntity}). */
    public static final int ITEM = 1 << 4;
    /** A {@code Mob} (has AI). */
    public static final int MOB = 1 << 5;
    /** An armor stand. */
    public static final int ARMOR_STAND = 1 << 6;
    /** Alive (not removed, not dead). */
    public static final int ALIVE = 1 << 7;
    /** A spectating player. */
    public static final int SPECTATOR = 1 << 8;
    /** A part of a multi-part entity (ender dragon parts). */
    public static final int PART = 1 << 9;
    /** A villager. */
    public static final int VILLAGER = 1 << 10;
}
