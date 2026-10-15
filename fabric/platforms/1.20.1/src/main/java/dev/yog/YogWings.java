package dev.yog;

import dev.yog.util.YogGlide;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.ElytraEntityModel;
import net.minecraft.client.render.entity.model.EntityModelLayers;
import net.minecraft.client.render.entity.model.EntityModelLoader;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.item.Items;
import net.minecraft.util.Identifier;

/**
 * A glider's wings ({@code client::set_glider} with a texture): an elytra
 * drawn with the mod's texture and a glint — unless the player wears a real
 * elytra.
 */
public class YogWings extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {
    private final ElytraEntityModel<AbstractClientPlayerEntity> model;

    public YogWings(FeatureRendererContext<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> ctx,
                    EntityModelLoader models) {
        super(ctx);
        this.model = new ElytraEntityModel<>(models.getModelPart(EntityModelLayers.ELYTRA));
    }

    @Override
    public void render(MatrixStack matrices, VertexConsumerProvider buffers, int light, AbstractClientPlayerEntity player,
                       float limbAngle, float limbDistance, float tickDelta, float age, float headYaw, float headPitch) {
        String wings = YogGlide.wings(player.getUuid());
        if (wings == null || player.getEquippedStack(EquipmentSlot.CHEST).isOf(Items.ELYTRA)) return;
        Identifier texture = Identifier.tryParse(wings);
        if (texture == null) return;
        matrices.push();
        matrices.translate(0.0F, 0.0F, 0.125F);
        getContextModel().copyStateTo(model);
        model.setAngles(player, limbAngle, limbDistance, age, headYaw, headPitch);
        VertexConsumer verts = ItemRenderer.getArmorGlintConsumer(buffers, RenderLayer.getArmorCutoutNoCull(texture), false, true);
        model.render(matrices, verts, light, OverlayTexture.DEFAULT_UV, 1.0F, 1.0F, 1.0F, 1.0F);
        matrices.pop();
    }
}
