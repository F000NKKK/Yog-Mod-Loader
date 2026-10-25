package dev.yog.script.host;

import dev.yog.script.Action1;
import dev.yog.script.Comparison;
import dev.yog.script.Converter;
import dev.yog.script.Predicate;
import dev.yog.script.collections.generic.ArrayView;
import dev.yog.script.collections.generic.Order;

/**
 * Yog Script's {@code System.Array}: every array's members (given the
 * value and its rank: Java can't tell {@code T[,]} from {@code T[][]}),
 * and its statics, one of each for Java's primitive arrays and one for
 * any other. What changes an array (taken {@code ref}) gives it back, the
 * caller's place taking it ({@code xs = Array.Sort(xs)}).
 */
public final class Array {
    private Array() {}

    // Members.

    public static int Length(java.lang.Object array, int rank) {
        int length = 1;
        for (int dimension = 0; dimension < rank; dimension++) {
            length *= GetLength(array, rank, dimension);
        }
        return length;
    }

    public static long LongLength(java.lang.Object array, int rank) {
        return Length(array, rank);
    }

    public static int Rank(java.lang.Object array, int rank) {
        return rank;
    }

    /** How many items dimension {@code dimension} holds: its first row's, down to it. */
    public static int GetLength(java.lang.Object array, int rank, int dimension) {
        if (dimension < 0 || dimension >= rank) {
            throw new IndexOutOfBoundsException("dimension " + dimension + " of an array of rank " + rank);
        }
        java.lang.Object row = array;
        for (int d = 0; d < dimension; d++) {
            if (java.lang.reflect.Array.getLength(row) == 0) {
                return 0;
            }
            row = java.lang.reflect.Array.get(row, 0);
        }
        return java.lang.reflect.Array.getLength(row);
    }

    // What generic code gave as an array of the type wanted.

    /** {@code items} (any array) as a new array of {@code type} ({@code int[].class}). */
    public static <A> A Of(java.lang.Object items, Class<A> type) {
        int length = java.lang.reflect.Array.getLength(items);
        java.lang.Object made = java.lang.reflect.Array.newInstance(type.getComponentType(), length);
        for (int i = 0; i < length; i++) {
            java.lang.reflect.Array.set(made, i, java.lang.reflect.Array.get(items, i));
        }
        return type.cast(made);
    }

    public static java.lang.Object[] Empty() {
        return new java.lang.Object[0];
    }

    // An array as its interfaces.
    public static ArrayView<Integer> AsList(int[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Long> AsList(long[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Short> AsList(short[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Byte> AsList(byte[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Character> AsList(char[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Float> AsList(float[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Double> AsList(double[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static ArrayView<Boolean> AsList(boolean[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }
    public static <T> ArrayView<T> AsList(T[] array) {
        return new ArrayView<>(array.length, i -> array[i], (i, v) -> array[i] = v);
    }

    // Sorted, searched.
    public static int[] Sort(int[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static int[] Sort(int[] array, Comparison<Integer> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static long[] Sort(long[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static long[] Sort(long[] array, Comparison<Long> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static short[] Sort(short[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static short[] Sort(short[] array, Comparison<Short> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static byte[] Sort(byte[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static byte[] Sort(byte[] array, Comparison<Byte> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static char[] Sort(char[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static char[] Sort(char[] array, Comparison<Character> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static float[] Sort(float[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static float[] Sort(float[] array, Comparison<Float> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static double[] Sort(double[] array) {
        java.util.Arrays.sort(array);
        return array;
    }
    public static double[] Sort(double[] array, Comparison<Double> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static boolean[] Sort(boolean[] array) {
        int falses = 0;
        for (boolean v : array) {
            falses += v ? 0 : 1;
        }
        java.util.Arrays.fill(array, 0, falses, false);
        java.util.Arrays.fill(array, falses, array.length, true);
        return array;
    }
    public static boolean[] Sort(boolean[] array, Comparison<Boolean> comparison) {
        AsList(array).Sort(comparison);
        return array;
    }
    public static <T> T[] Sort(T[] array) {
        java.util.Arrays.sort(array, Order.natural());
        return array;
    }

    public static <T> T[] Sort(T[] array, Comparison<T> comparison) {
        java.util.Arrays.sort(array, comparison::Invoke);
        return array;
    }
    public static int IndexOf(int[] array, int value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(int[] array, int value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(int[] array, int value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(long[] array, long value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(long[] array, long value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(long[] array, long value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(short[] array, short value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(short[] array, short value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(short[] array, short value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(byte[] array, byte value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(byte[] array, byte value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(byte[] array, byte value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(char[] array, char value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(char[] array, char value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(char[] array, char value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(float[] array, float value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(float[] array, float value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(float[] array, float value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(double[] array, double value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(double[] array, double value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(double[] array, double value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static int IndexOf(boolean[] array, boolean value) {
        return AsList(array).IndexOf(value);
    }

    public static int LastIndexOf(boolean[] array, boolean value) {
        return AsList(array).LastIndexOf(value);
    }

    public static int BinarySearch(boolean[] array, boolean value) {
        return ArrayView.binarySearch(AsList(array), value, Order.natural());
    }
    public static <T> int IndexOf(T[] array, T value) {
        return AsList(array).IndexOf(value);
    }

    public static <T> int LastIndexOf(T[] array, T value) {
        return AsList(array).LastIndexOf(value);
    }

    public static <T> int BinarySearch(T[] array, T value) {
        return java.util.Arrays.binarySearch(array, value, Order.natural());
    }

    // Changed.
    public static int[] Reverse(int[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static int[] Fill(int[] array, int value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static int[] Clear(int[] array) {
        return Fill(array, 0);
    }

    public static int[] Copy(int[] sourceArray, int[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static int[] Resize(int[] array, int newSize) {
        return array == null ? new int[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static long[] Reverse(long[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static long[] Fill(long[] array, long value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static long[] Clear(long[] array) {
        return Fill(array, 0L);
    }

    public static long[] Copy(long[] sourceArray, long[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static long[] Resize(long[] array, int newSize) {
        return array == null ? new long[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static short[] Reverse(short[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static short[] Fill(short[] array, short value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static short[] Clear(short[] array) {
        return Fill(array, (short) 0);
    }

    public static short[] Copy(short[] sourceArray, short[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static short[] Resize(short[] array, int newSize) {
        return array == null ? new short[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static byte[] Reverse(byte[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static byte[] Fill(byte[] array, byte value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static byte[] Clear(byte[] array) {
        return Fill(array, (byte) 0);
    }

    public static byte[] Copy(byte[] sourceArray, byte[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static byte[] Resize(byte[] array, int newSize) {
        return array == null ? new byte[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static char[] Reverse(char[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static char[] Fill(char[] array, char value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static char[] Clear(char[] array) {
        return Fill(array, '\0');
    }

    public static char[] Copy(char[] sourceArray, char[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static char[] Resize(char[] array, int newSize) {
        return array == null ? new char[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static float[] Reverse(float[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static float[] Fill(float[] array, float value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static float[] Clear(float[] array) {
        return Fill(array, 0f);
    }

    public static float[] Copy(float[] sourceArray, float[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static float[] Resize(float[] array, int newSize) {
        return array == null ? new float[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static double[] Reverse(double[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static double[] Fill(double[] array, double value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static double[] Clear(double[] array) {
        return Fill(array, 0d);
    }

    public static double[] Copy(double[] sourceArray, double[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static double[] Resize(double[] array, int newSize) {
        return array == null ? new double[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static boolean[] Reverse(boolean[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static boolean[] Fill(boolean[] array, boolean value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static boolean[] Clear(boolean[] array) {
        return Fill(array, false);
    }

    public static boolean[] Copy(boolean[] sourceArray, boolean[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    public static boolean[] Resize(boolean[] array, int newSize) {
        return array == null ? new boolean[newSize] : java.util.Arrays.copyOf(array, newSize);
    }
    public static <T> T[] Reverse(T[] array) {
        AsList(array).Reverse();
        return array;
    }

    public static <T> T[] Fill(T[] array, T value) {
        java.util.Arrays.fill(array, value);
        return array;
    }

    public static <T> T[] Clear(T[] array) {
        return Fill(array, null);
    }

    public static <T> T[] Copy(T[] sourceArray, T[] destinationArray, int length) {
        System.arraycopy(sourceArray, 0, destinationArray, 0, length);
        return destinationArray;
    }

    /** {@code array} made {@code newSize} long; a null one an array of objects, Java not knowing its type. */
    @SuppressWarnings("unchecked")
    public static <T> T[] Resize(T[] array, int newSize) {
        return array == null ? (T[]) new java.lang.Object[newSize] : java.util.Arrays.copyOf(array, newSize);
    }

    // Matched.
    public static boolean Exists(int[] array, Predicate<Integer> match) {
        return AsList(array).Exists(match);
    }

    public static Integer Find(int[] array, Predicate<Integer> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(int[] array, Predicate<Integer> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(int[] array, Predicate<Integer> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(int[] array, Predicate<Integer> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(int[] array, Action1<Integer> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(int[] array, Converter<Integer, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(long[] array, Predicate<Long> match) {
        return AsList(array).Exists(match);
    }

    public static Long Find(long[] array, Predicate<Long> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(long[] array, Predicate<Long> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(long[] array, Predicate<Long> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(long[] array, Predicate<Long> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(long[] array, Action1<Long> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(long[] array, Converter<Long, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(short[] array, Predicate<Short> match) {
        return AsList(array).Exists(match);
    }

    public static Short Find(short[] array, Predicate<Short> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(short[] array, Predicate<Short> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(short[] array, Predicate<Short> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(short[] array, Predicate<Short> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(short[] array, Action1<Short> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(short[] array, Converter<Short, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(byte[] array, Predicate<Byte> match) {
        return AsList(array).Exists(match);
    }

    public static Byte Find(byte[] array, Predicate<Byte> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(byte[] array, Predicate<Byte> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(byte[] array, Predicate<Byte> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(byte[] array, Predicate<Byte> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(byte[] array, Action1<Byte> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(byte[] array, Converter<Byte, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(char[] array, Predicate<Character> match) {
        return AsList(array).Exists(match);
    }

    public static Character Find(char[] array, Predicate<Character> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(char[] array, Predicate<Character> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(char[] array, Predicate<Character> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(char[] array, Predicate<Character> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(char[] array, Action1<Character> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(char[] array, Converter<Character, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(float[] array, Predicate<Float> match) {
        return AsList(array).Exists(match);
    }

    public static Float Find(float[] array, Predicate<Float> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(float[] array, Predicate<Float> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(float[] array, Predicate<Float> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(float[] array, Predicate<Float> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(float[] array, Action1<Float> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(float[] array, Converter<Float, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(double[] array, Predicate<Double> match) {
        return AsList(array).Exists(match);
    }

    public static Double Find(double[] array, Predicate<Double> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(double[] array, Predicate<Double> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(double[] array, Predicate<Double> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(double[] array, Predicate<Double> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(double[] array, Action1<Double> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(double[] array, Converter<Double, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static boolean Exists(boolean[] array, Predicate<Boolean> match) {
        return AsList(array).Exists(match);
    }

    public static Boolean Find(boolean[] array, Predicate<Boolean> match) {
        return AsList(array).Find(match);
    }

    public static java.lang.Object[] FindAll(boolean[] array, Predicate<Boolean> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static int FindIndex(boolean[] array, Predicate<Boolean> match) {
        return AsList(array).FindIndex(match);
    }

    public static boolean TrueForAll(boolean[] array, Predicate<Boolean> match) {
        return AsList(array).TrueForAll(match);
    }

    public static void ForEach(boolean[] array, Action1<Boolean> action) {
        AsList(array).ForEach(action);
    }

    public static <O> java.lang.Object[] ConvertAll(boolean[] array, Converter<Boolean, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
    public static <T> boolean Exists(T[] array, Predicate<T> match) {
        return AsList(array).Exists(match);
    }

    public static <T> T Find(T[] array, Predicate<T> match) {
        return AsList(array).Find(match);
    }

    public static <T> java.lang.Object[] FindAll(T[] array, Predicate<T> match) {
        return AsList(array).FindAll(match).toArray();
    }

    public static <T> int FindIndex(T[] array, Predicate<T> match) {
        return AsList(array).FindIndex(match);
    }

    public static <T> boolean TrueForAll(T[] array, Predicate<T> match) {
        return AsList(array).TrueForAll(match);
    }

    public static <T> void ForEach(T[] array, Action1<T> action) {
        AsList(array).ForEach(action);
    }

    public static <I, O> java.lang.Object[] ConvertAll(I[] array, Converter<I, O> converter) {
        return AsList(array).ConvertAll(converter);
    }
}
