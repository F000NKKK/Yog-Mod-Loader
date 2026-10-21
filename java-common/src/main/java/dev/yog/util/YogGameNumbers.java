package dev.yog.util;

/** The game's own numbers the hosts' entities, blocks and drawings copy — the same on every version. */
public final class YogGameNumbers {
    private YogGameNumbers() {}

    /** A falling thing's speed gained a tick, and what its speed keeps a tick in the air and on the ground. */
    public static final double GRAVITY = 0.04;
    public static final double AIR_DRAG = 0.98;
    public static final double GROUND_DRAG = 0.5;

    /** How far (squared, in blocks) a player may be from a container to use it: 8 blocks. */
    public static final double CONTAINER_REACH_SQR = 64.0;

    /** A mob's goals: how fast it panics; whom it avoids, how far, and how fast it walks and runs away. */
    public static final double PANIC_SPEED = 1.25;
    public static final float AVOID_DISTANCE = 8.0F;
    public static final double AVOID_WALK = 1.0;
    public static final double AVOID_RUN = 1.2;

    /** A hanging thing (a painting's): its thickness, and how far its middle sits back from the block's. */
    public static final double HANGING_THICKNESS = 1.0 / 16;
    public static final double HANGING_DEPTH = 0.5 - 1.0 / 32;

    /** Pixels an item's picture is wide. */
    public static final float ITEM_PIXELS = 16.0f;
    /** An entity drawn in a box: fit to its height, or its width with room for its arms, a little smaller, never by a size of 0. */
    public static final float ENTITY_ARMS = 1.4f;
    public static final float ENTITY_MARGIN = 0.9f;
    public static final float ENTITY_MIN_SIZE = 0.1f;
    /** A block drawn in a box: its picture's size to the box's (a cube's corners stick out). */
    public static final float BLOCK_CORNERS = 1.7f;
}
