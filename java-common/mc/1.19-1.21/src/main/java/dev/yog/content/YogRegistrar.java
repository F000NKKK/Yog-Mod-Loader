package dev.yog.content;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import net.minecraft.core.Registry;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.resources.ResourceKey;

/**
 * What the mods register, by registry, in the order the game's registries
 * come: Fabric registers it all at once ({@link #runAll}), NeoForge and
 * Forge one registry at a time, as each one's register event comes
 * ({@link #run}). The shared host code adds to it; a loader only says how a
 * thing goes into a registry ({@link Sink}).
 */
public final class YogRegistrar {
    private YogRegistrar() {}

    /** How a loader puts {@code value} into {@code registry} as {@code id}. */
    public interface Sink {
        <T> void register(ResourceKey<? extends Registry<T>> registry, ResourceLocation id, T value);
    }

    /** What goes into each registry, in the order it was asked for. */
    private static final Map<ResourceKey<? extends Registry<?>>, List<Consumer<Sink>>> STEPS = new LinkedHashMap<>();

    /** {@code step} registers into {@code registry} when its turn comes. */
    public static void add(ResourceKey<? extends Registry<?>> registry, Consumer<Sink> step) {
        STEPS.computeIfAbsent(registry, k -> new ArrayList<>()).add(step);
    }

    /** Every registry's steps, in the order they were added (Fabric). */
    public static void runAll(Sink sink) {
        STEPS.values().forEach(steps -> steps.forEach(s -> s.accept(sink)));
    }

    /** {@code registry}'s steps (its register event, on NeoForge and Forge). */
    public static void run(ResourceKey<? extends Registry<?>> registry, Sink sink) {
        STEPS.getOrDefault(registry, List.of()).forEach(s -> s.accept(sink));
    }
}
