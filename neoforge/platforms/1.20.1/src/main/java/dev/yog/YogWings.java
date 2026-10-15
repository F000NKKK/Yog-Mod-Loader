package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import dev.yog.util.YogGlide;
import net.minecraft.client.model.ElytraModel;
import net.minecraft.client.model.PlayerModel;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.model.geom.ModelLayers;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.RenderType;
import net.minecraft.client.renderer.entity.ItemRenderer;
import net.minecraft.client.renderer.entity.RenderLayerParent;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.player.PlayerRenderer;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.Items;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.EntityRenderersEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

/**
 * A glider's wings ({@code client::set_glider} with a texture): an elytra
 * drawn with the mod's texture and a glint — unless the player wears a real
 * elytra.
 */
public class YogWings extends RenderLayer<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> {
    private final ElytraModel<AbstractClientPlayer> model;

    public YogWings(RenderLayerParent<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> parent, EntityModelSet models) {
        super(parent);
        this.model = new ElytraModel<>(models.bakeLayer(ModelLayers.ELYTRA));
    }

    @Override
    public void render(PoseStack pose, MultiBufferSource buffers, int light, AbstractClientPlayer player,
                       float limbSwing, float limbSwingAmount, float partialTick, float age, float headYaw, float headPitch) {
        String wings = YogGlide.wings(player.getUUID());
        if (wings == null || player.getItemBySlot(EquipmentSlot.CHEST).is(Items.ELYTRA)) return;
        ResourceLocation texture = ResourceLocation.tryParse(wings);
        if (texture == null) return;
        pose.pushPose();
        pose.translate(0.0F, 0.0F, 0.125F);
        getParentModel().copyPropertiesTo(model);
        model.setupAnim(player, limbSwing, limbSwingAmount, age, headYaw, headPitch);
        VertexConsumer verts = ItemRenderer.getArmorFoilBuffer(buffers, RenderType.armorCutoutNoCull(texture), false, true);
        model.renderToBuffer(pose, verts, light, OverlayTexture.NO_OVERLAY, 1.0F, 1.0F, 1.0F, 1.0F);
        pose.popPose();
    }

    /** Puts the wings on every player skin. */
    @Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.MOD, value = Dist.CLIENT)
    public static final class Setup {
        private Setup() {}

        @SubscribeEvent
        public static void onAddLayers(EntityRenderersEvent.AddLayers event) {
            for (var name : event.getSkins()) {
                Object skin = event.getSkin(name);
                if (skin instanceof PlayerRenderer players) players.addLayer(new YogWings(players, event.getEntityModels()));
            }
        }
    }
}
