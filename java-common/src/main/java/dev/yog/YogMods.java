package dev.yog;

import dev.yog.bus.YogTypes;
import java.util.List;

/** The loader's own (Java) mods, for the bus's {@code game/platform_mods}. */
public final class YogMods {
    private YogMods() {}

    @YogServiceHost
    public static List<YogTypes.ModInfo> platformMods() {
        return YogPlatform.mods().stream()
                .map(m -> new YogTypes.ModInfo(YogNames.ModSource.PLATFORM, m.id(), m.name(), m.version(), m.authors(), m.description()))
                .toList();
    }
}
