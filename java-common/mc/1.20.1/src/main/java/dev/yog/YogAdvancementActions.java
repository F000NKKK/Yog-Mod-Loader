package dev.yog;

import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/** The host's advancement actions: the methods {@code yog_event::advancement} names. */
public final class YogAdvancementActions {
    private YogAdvancementActions() {}

    /** Awards advancement {@code id} to the player {@code uuid}: every criterion it has left. */
    @YogServiceHost
    public static boolean advancementGrant(String uuid, String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null || id == null || !(NativeBridge.entityByUuid(uuid) instanceof net.minecraft.server.level.ServerPlayer p)) return false;
        net.minecraft.advancements.Advancement adv = s.getAdvancements().getAdvancement(net.minecraft.resources.ResourceLocation.tryParse(id));
        if (adv == null) return false;
        var progress = p.getAdvancements().getOrStartProgress(adv);
        java.util.List<String> left = new java.util.ArrayList<>();
        progress.getRemainingCriteria().forEach(left::add);
        for (String c : left) p.getAdvancements().award(adv, c);
        return true;
    }

    /** Whether the player {@code uuid} has advancement {@code id}. */
    @YogServiceHost
    public static boolean advancementDone(String uuid, String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null || id == null || !(NativeBridge.entityByUuid(uuid) instanceof net.minecraft.server.level.ServerPlayer p)) return false;
        net.minecraft.advancements.Advancement adv = s.getAdvancements().getAdvancement(net.minecraft.resources.ResourceLocation.tryParse(id));
        return adv != null && p.getAdvancements().getOrStartProgress(adv).isDone();
    }
}
