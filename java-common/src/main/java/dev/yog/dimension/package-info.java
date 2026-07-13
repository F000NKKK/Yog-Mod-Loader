/**
 * Yog Dimension Framework — abstractions for working with Minecraft dimensions.
 *
 * <p>This package defines the API that mods use to read existing dimensions
 * and create new ones. Implementations live in platform-specific modules
 * (fabric, forge, neoforge).</p>
 *
 * <p>Key types:
 * <ul>
 *   <li>{@link dev.yog.dimension.YogDimension} — a dimension handle</li>
 *   <li>{@link dev.yog.dimension.YogDimensionType} — dimension type properties</li>
 *   <li>{@link dev.yog.dimension.YogChunkGenerator} — terrain generation</li>
 *   <li>{@link dev.yog.dimension.YogWeather} — weather/precipitation effects</li>
 *   <li>{@link dev.yog.dimension.YogSkyEffect} — sky rendering</li>
 * </ul>
 */
package dev.yog.dimension;