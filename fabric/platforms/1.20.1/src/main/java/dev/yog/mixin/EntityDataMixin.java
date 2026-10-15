package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.entity.Entity;
import net.minecraft.nbt.NbtCompound;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Entity data a mod marked {@code persist} is saved and loaded with the entity. */
@Mixin(Entity.class)
public abstract class EntityDataMixin {
    private static final String KEY = "yog:data";

    @Inject(method = "writeNbt", at = @At("RETURN"))
    private void yog$save(NbtCompound tag, CallbackInfoReturnable<NbtCompound> cir) {
        Entity self = (Entity) (Object) this;
        if (self.getWorld() == null || self.getWorld().isClient) return;
        byte[] data = NativeBridge.nativeEntityDataSave(self.getUuidAsString());
        if (data != null) cir.getReturnValue().putByteArray(KEY, data);
    }

    @Inject(method = "readNbt", at = @At("TAIL"))
    private void yog$load(NbtCompound tag, CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        if (self.getWorld() == null || self.getWorld().isClient || !tag.contains(KEY)) return;
        NativeBridge.nativeEntityDataLoad(self.getUuidAsString(), tag.getByteArray(KEY));
    }
}
