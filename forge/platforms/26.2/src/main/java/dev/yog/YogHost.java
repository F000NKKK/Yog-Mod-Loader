package dev.yog;

import dev.yog.content.YogContentRegistry;
import dev.yog.content.YogEffects;
import dev.yog.content.YogEntityTypes;
import dev.yog.content.YogPackProvider;
import dev.yog.content.YogRegistrar;
import dev.yog.server.YogCommands;
import dev.yog.server.YogServerEvents;

import net.minecraft.core.Registry;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.level.Level;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.event.AddPackFindersEvent;
import net.minecraftforge.event.RegisterCommandsEvent;
import net.minecraftforge.event.ServerChatEvent;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.event.brewing.BrewingRecipeRegisterEvent;
import net.minecraftforge.event.entity.EntityAttributeCreationEvent;
import net.minecraftforge.event.entity.EntityAttributeModificationEvent;
import net.minecraftforge.event.entity.EntityJoinLevelEvent;
import net.minecraftforge.event.entity.EntityLeaveLevelEvent;
import net.minecraftforge.event.entity.living.LivingAttackEvent;
import net.minecraftforge.event.entity.living.LivingDeathEvent;
import net.minecraftforge.event.entity.player.AttackEntityEvent;
import net.minecraftforge.event.entity.player.PlayerEvent;
import net.minecraftforge.event.entity.player.PlayerInteractEvent;
import net.minecraftforge.event.furnace.FurnaceFuelBurnTimeEvent;
import net.minecraftforge.event.level.BlockEvent;
import net.minecraftforge.event.server.ServerStartedEvent;
import net.minecraftforge.event.server.ServerStoppingEvent;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.fml.loading.FMLEnvironment;
import net.minecraftforge.registries.RegisterEvent;

/**
 * Forge's entry point: boots the runtime, registers the mods' content
 * ({@link YogContentRegistry}) registry by registry, and hooks Forge's
 * events up to {@link YogServerEvents}. All it knows is Forge.
 */
@Mod(YogNames.Ns.YOG)
public final class YogHost {
    public YogHost(FMLJavaModLoadingContext context) {
        NativeBridge.ensureLoaded();
        YogLog.info("Forge host initialised.");
        YogContentRegistry.schedule();
        YogPlatform.openChannel();

        var mods = context.getModBusGroup();
        RegisterEvent.getBus(mods).addListener(YogHost::register);
        AddPackFindersEvent.BUS.addListener(e -> e.addRepositorySource(new YogPackProvider(e.getPackType())));
        EntityAttributeCreationEvent.BUS.addListener(e -> YogEntityTypes.attributes((type, a) -> e.put(type, a.build())));
        EntityAttributeModificationEvent.BUS.addListener(e -> YogEffects.PLAYER_ATTRIBUTES.forEach(a -> e.add(EntityTypes.PLAYER, a)));

        hookServerEvents();
        if (FMLEnvironment.dist == Dist.CLIENT) YogClient.init(mods);
    }

    /** A register event: the mods' steps for its registry. */
    private static void register(RegisterEvent event) {
        YogRegistrar.run(event.getRegistryKey(), new YogRegistrar.Sink() {
            @Override
            public <T> void register(ResourceKey<? extends Registry<T>> registry, Identifier id, T value) {
                event.register(registry, id, () -> value);
            }
        });
    }

    private static void hookServerEvents() {
        ServerStartedEvent.BUS.addListener(e -> YogServerEvents.serverStarted(e.getServer()));
        ServerStoppingEvent.BUS.addListener(e -> YogServerEvents.serverStopping());
        TickEvent.ServerTickEvent.Post.BUS.addListener(e -> YogServerEvents.serverTick(e.server()));
        RegisterCommandsEvent.BUS.addListener(e -> YogCommands.register(e.getDispatcher()));
        FurnaceFuelBurnTimeEvent.BUS.addListener(e -> {
            Integer ticks = YogContentRegistry.FUELS.get(e.getItemStack().getItem());
            if (ticks != null) e.setBurnTime(ticks);
        });
        BrewingRecipeRegisterEvent.BUS.addListener(e -> YogEffects.mixes(e.getBuilder()::addMix));

        PlayerEvent.PlayerLoggedInEvent.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerJoined(sp);
        });
        PlayerEvent.PlayerLoggedOutEvent.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerLeft(sp);
        });
        PlayerEvent.PlayerRespawnEvent.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.respawned(sp, e.isEndConquered());
        });
        PlayerEvent.PlayerChangedDimensionEvent.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.changedLevel(sp);
        });
        PlayerEvent.StartTracking.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.startedTracking(e.getTarget(), sp);
        });

        // A cancelling listener answers whether to cancel.
        BlockEvent.BreakEvent.BUS.addListener(e -> {
            if (!(e.getLevel() instanceof Level level) || level.isClientSide()) return false;
            if (!YogServerEvents.blockBreakPre(level, e.getPlayer(), e.getPos(), e.getState())) return true;
            YogServerEvents.blockBroken(level, e.getPlayer(), e.getPos(), e.getState(), level.getBlockEntity(e.getPos()));
            return false;
        });
        ServerChatEvent.BUS.addListener(e -> {
            if (!YogServerEvents.chatPre(e.getPlayer(), e.getRawText())) return true;
            YogServerEvents.chat(e.getPlayer(), e.getRawText());
            return false;
        });
        PlayerInteractEvent.RightClickItem.BUS.addListener(e ->
                e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useItem(sp, e.getHand()));
        PlayerInteractEvent.RightClickBlock.BUS.addListener(e ->
                e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useBlock(sp, e.getHand(), e.getHitVec()));
        PlayerInteractEvent.EntityInteractSpecific.BUS.addListener(e ->
                e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.interactEntity(sp, e.getHand(), e.getTarget()));
        AttackEntityEvent.BUS.addListener(e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.attackEntity(sp, e.getTarget());
            return false;
        });
        LivingAttackEvent.BUS.addListener(e ->
                !e.getEntity().level().isClientSide() && !YogServerEvents.damage(e.getEntity(), e.getSource(), e.getAmount()));
        LivingDeathEvent.BUS.addListener(e -> {
            if (e.getEntity().level().isClientSide()) return false;
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.playerDeathPre(sp, e.getSource())) return true;
            YogServerEvents.died(e.getEntity(), e.getSource());
            return false;
        });
        EntityJoinLevelEvent.BUS.addListener(e ->
                !e.getLevel().isClientSide() && !YogServerEvents.entityLoaded(e.getEntity(), e.getLevel()));
        EntityLeaveLevelEvent.BUS.addListener(e -> {
            if (!e.getLevel().isClientSide()) YogServerEvents.entityUnloaded(e.getEntity());
        });
    }
}
