package dev.yog.block;

import dev.yog.YogNames;

/**
 * How a Yog block with states turns: the same turn its blockstate gives its
 * model (`yog-cli`'s {@code turn}), so its shape turns with what's drawn.
 * Free of game types: each host feeds it names.
 */
public final class YogStateLogic {
    private YogStateLogic() {}

    /**
     * The model's turn, x then y, in degrees: {@code facing} is where its
     * front (the model's north side) looks; {@code face} ({@code floor},
     * {@code wall}, {@code ceiling} or null), where an attached block — its
     * model lying on the floor — hangs.
     */
    public static int[] turn(String facing, String face) {
        int yaw = switch (facing == null ? "" : facing) {
            case YogNames.Direction.EAST -> 90;
            case YogNames.Direction.SOUTH -> 180;
            case YogNames.Direction.WEST -> 270;
            default -> 0;
        };
        if (YogNames.BlockState.WALL.equals(face)) return new int[] {90, yaw};
        if (YogNames.BlockState.CEILING.equals(face)) return new int[] {180, (yaw + 180) % 360};
        if (YogNames.Direction.UP.equals(facing)) return new int[] {270, 0};
        if (YogNames.Direction.DOWN.equals(facing)) return new int[] {90, 0};
        return new int[] {0, facing == null ? 0 : yaw};
    }

    /** A box ({@code x1, y1, z1, x2, y2, z2} in pixels) turned as a model is: x, then y. */
    public static double[] turnBox(double[] box, int x, int y) {
        double[] a = {box[0], box[1], box[2]};
        double[] b = {box[3], box[4], box[5]};
        for (int i = 0; i < x / 90; i++) {
            a = aboutX(a);
            b = aboutX(b);
        }
        for (int i = 0; i < y / 90; i++) {
            a = aboutY(a);
            b = aboutY(b);
        }
        return new double[] {
            Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.min(a[2], b[2]),
            Math.max(a[0], b[0]), Math.max(a[1], b[1]), Math.max(a[2], b[2]),
        };
    }

    /** A quarter turn about x, as a model's {@code x: 90}: its north side goes down. */
    private static double[] aboutX(double[] p) {
        return new double[] {p[0], p[2], 16 - p[1]};
    }

    /** A quarter turn about y, as a model's {@code y: 90}: north goes east. */
    private static double[] aboutY(double[] p) {
        return new double[] {16 - p[2], p[1], p[0]};
    }
}
