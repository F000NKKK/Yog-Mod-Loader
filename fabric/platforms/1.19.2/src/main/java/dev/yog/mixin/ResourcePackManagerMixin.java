package dev.yog.mixin;

import dev.yog.YogLog;

import java.util.HashSet;
import java.util.Set;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import dev.yog.content.YogPackProvider;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Adds the Yog pack provider to every resource/data pack manager, so the assets
 * and data bundled in .yog mods are served to the client and the server.
 */
@Mixin(PackRepository.class)
public class ResourcePackManagerMixin {
    @Shadow
    @Final
    @Mutable
    private Set<RepositorySource> sources;

    /** The repository of a type (the client's resources, the server's data) — this version names it. */
    @Inject(method = "<init>(Lnet/minecraft/server/packs/PackType;[Lnet/minecraft/server/packs/repository/RepositorySource;)V",
            at = @At("TAIL"))
    private void yog$addProvider(PackType type, RepositorySource[] providers, CallbackInfo ci) {
        try {
            Set<RepositorySource> updated = new HashSet<>(this.sources);
            updated.add(new YogPackProvider(type));
            this.sources = updated;
        } catch (Throwable t) {
            YogLog.warn("could not register pack provider: " + t);
        }
    }
}
