package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundSource;

/** The host's sound actions: the methods {@code yog_event::sound} names. */
public final class YogSoundActions {
    private YogSoundActions() {}

    @YogServiceHost
    public static boolean playSound(String dimension, double x, double y, double z, String soundId, float volume, float pitch) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        ResourceLocation id = ResourceLocation.tryParse(soundId);
        if (w == null || id == null) return false;
        w.playSound(null, x, y, z, net.minecraft.core.registries.BuiltInRegistries.SOUND_EVENT.get(id), SoundSource.MASTER, volume, pitch);
        return true;
    }

    // ── sound / boss bar ────────────────────────────────────────────────────

    @YogServiceHost
    public static boolean playSoundToPlayer(String playerName, String soundId, float volume, float pitch) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        ResourceLocation id = ResourceLocation.tryParse(soundId);
        if (p == null || id == null) return false;
        p.serverLevel().playSound(null, p.getX(), p.getY(), p.getZ(),
                net.minecraft.sounds.SoundEvent.createVariableRangeEvent(id), SoundSource.MASTER, volume, pitch);
        return true;
    }
}
