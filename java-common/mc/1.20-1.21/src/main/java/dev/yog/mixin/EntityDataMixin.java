package dev.yog.mixin;

import dev.yog.YogNames;

import dev.yog.NativeBridge;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Entity data a mod marked {@code persist} is saved and loaded with the entity. */
@Mixin(Entity.class)
public abstract class EntityDataMixin {
    private static final String KEY = YogNames.Yog.ENTITY_NBT;

    @Inject(method = "saveWithoutId", at = @At("RETURN"))
    private void yog$save(CompoundTag tag, CallbackInfoReturnable<CompoundTag> cir) {
        Entity self = (Entity) (Object) this;
        if (self.level() == null || self.level().isClientSide) return;
        byte[] data = NativeBridge.nativeEntityDataSave(self.getStringUUID());
        if (data != null) cir.getReturnValue().putByteArray(KEY, data);
    }

    @Inject(method = "load", at = @At("TAIL"))
    private void yog$load(CompoundTag tag, CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        if (self.level() == null || self.level().isClientSide || !tag.contains(KEY)) return;
        NativeBridge.nativeEntityDataLoad(self.getStringUUID(), tag.getByteArray(KEY));
    }
}
