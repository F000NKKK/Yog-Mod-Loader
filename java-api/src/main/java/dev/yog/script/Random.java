package dev.yog.script;

/**
 * Numbers drawn at random: Yog Script's {@code System.Random}, seeded as
 * .NET's (the same seed draws the same numbers here). What changes an
 * array it's handed {@code ref} gives it back.
 */
public class Random {
    private static final Random SHARED = new Random();

    private final java.util.Random rng;

    public Random() {
        rng = new java.util.Random();
    }

    public Random(int seed) {
        rng = new java.util.Random(seed);
    }

    public static Random Shared() {
        return SHARED;
    }

    public int Next() {
        return rng.nextInt(Integer.MAX_VALUE);
    }

    public int Next(int maxValue) {
        return Next(0, maxValue);
    }

    /** From {@code minValue} (in) up to {@code maxValue} (out); {@code minValue} itself when they're level. */
    public int Next(int minValue, int maxValue) {
        if (minValue > maxValue) {
            throw new IllegalArgumentException("`minValue` " + minValue + " is above `maxValue` " + maxValue);
        }
        return minValue == maxValue ? minValue : rng.nextInt(minValue, maxValue);
    }

    public long NextInt64() {
        return rng.nextLong(Long.MAX_VALUE);
    }

    public long NextInt64(long minValue, long maxValue) {
        if (minValue > maxValue) {
            throw new IllegalArgumentException("`minValue` " + minValue + " is above `maxValue` " + maxValue);
        }
        return minValue == maxValue ? minValue : rng.nextLong(minValue, maxValue);
    }

    public double NextDouble() {
        return rng.nextDouble();
    }

    public float NextSingle() {
        return rng.nextFloat();
    }

    public int[] Shuffle(int[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            int swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public long[] Shuffle(long[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            long swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public short[] Shuffle(short[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            short swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public byte[] Shuffle(byte[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            byte swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public char[] Shuffle(char[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            char swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public float[] Shuffle(float[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            float swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public double[] Shuffle(double[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            double swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public boolean[] Shuffle(boolean[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            boolean swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }

    public <T> T[] Shuffle(T[] values) {
        for (int i = values.length - 1; i > 0; i--) {
            int j = rng.nextInt(i + 1);
            T swapped = values[i];
            values[i] = values[j];
            values[j] = swapped;
        }
        return values;
    }
}
