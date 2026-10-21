package dev.yog;

import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;

/** The host's sound actions: the methods {@code yog_event::sound} names. */
public final class YogSoundActions {
    private YogSoundActions() {}

    @YogServiceHost
    public static boolean playSound(
            String dimension, double x, double y, double z,
            String soundId, float volume, float pitch) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        Identifier id = Identifier.tryParse(soundId);
        if (w == null || id == null) return false;
        w.playSound(null, x, y, z, SoundEvent.createVariableRangeEvent(id), SoundSource.MASTER, volume, pitch);
        return true;
    }

    @YogServiceHost
    public static boolean playSoundToPlayer(String playerName, String soundId, float volume, float pitch) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        Identifier id = Identifier.tryParse(soundId);
        if (p == null || id == null) return false;
        p.level().playSound(
                null, p.getX(), p.getY(), p.getZ(),
                SoundEvent.createVariableRangeEvent(id), SoundSource.MASTER, volume, pitch);
        return true;
    }
}
