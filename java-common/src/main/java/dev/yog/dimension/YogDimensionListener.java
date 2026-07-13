package dev.yog.dimension;

/**
 * Listener for dimension events — fires when properties change,
 * weather changes, players enter/leave, etc.
 *
 * <p>Register via {@link YogDimension#addListener(YogDimensionListener)}.</p>
 */
public interface YogDimensionListener {

    /** A property was changed. */
    default void onPropertyChanged(YogDimension dim, String key, String oldValue, String newValue) {}

    /** Weather state changed. */
    default void onWeatherChanged(YogDimension dim, boolean raining, boolean thundering) {}

    /** A player entered the dimension. */
    default void onPlayerEnter(YogDimension dim, String playerName, String playerUuid) {}

    /** A player left the dimension. */
    default void onPlayerLeave(YogDimension dim, String playerName, String playerUuid) {}

    /** The dimension was loaded (world created). */
    default void onLoaded(YogDimension dim) {}

    /** The dimension is about to be unloaded. */
    default void onUnloading(YogDimension dim) {}
}