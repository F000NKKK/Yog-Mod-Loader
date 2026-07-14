/**
 * Yog Dimension Framework — abstractions for working with Minecraft dimensions.
 *
 * <p>This package defines the API that mods use to read existing dimensions
 * and create new ones. Implementations live in platform-specific modules
 * (fabric, forge, neoforge).</p>
 *
 * <p>Two-phase lifecycle, mirroring how blocks/items are already registered:
 * <ul>
 *   <li><b>Declare</b> the dimension type at mod-init time (same window as
 *       block/item registration) — on the Rust side, {@code
 *       Registry::register_dimension}, which the host parses into a
 *       {@link dev.yog.dimension.YogDimensionDef} here.</li>
 *   <li><b>Create</b> the actual world at any point at runtime — no fixed
 *       chunk-generator preset is provided by this framework; mods write
 *       their own terrain logic as a Rust closure ({@code
 *       Registry::register_chunk_generator} in {@code yog-dimensions}),
 *       called once per chunk column. This package has no Java-side
 *       "generator config" type — the platform host just forwards each
 *       chunk request to the mod's registered callback.</li>
 * </ul>
 *
 * <p>Key types:
 * <ul>
 *   <li>{@link dev.yog.dimension.YogDimension} — a dimension handle</li>
 *   <li>{@link dev.yog.dimension.YogDimensionType} — dimension type properties</li>
 *   <li>{@link dev.yog.dimension.YogDimensionDef} — what a mod registers to
 *       declare a custom dimension type</li>
 * </ul>
 *
 * <p>Not implemented yet (separate follow-up):
 * {@code YogWeather}, {@code YogSkyEffect}, {@code YogDimensionEvent}.</p>
 */
package dev.yog.dimension;