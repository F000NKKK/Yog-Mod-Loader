package dev.yog;

import net.minecraft.core.Registry;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.AddPackFindersEvent;
import net.minecraftforge.event.RegisterCommandsEvent;
import net.minecraftforge.event.ServerChatEvent;
import net.minecraftforge.event.TickEvent;
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
import net.minecraftforge.fml.event.lifecycle.FMLCommonSetupEvent;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.fml.loading.FMLEnvironment;
import net.minecraftforge.registries.RegisterEvent;

/**
 * Forge's entry point (NeoForge 1.20.1's too: Forge 47's API): boots the runtime, registers the mods' content
 * ({@link YogContentRegistry}) registry by registry, and hooks Forge's
 * events up to {@link YogServerEvents}. All it knows is Forge.
 */
@Mod(YogNames.Ns.YOG)
public final class YogHost {
    public YogHost() {
        NativeBridge.ensureLoaded();
        YogLog.info(YogLoader.NAME + " host initialised.");
        YogContentRegistry.schedule();
        YogPlatform.openChannel();

        var modBus = FMLJavaModLoadingContext.get().getModEventBus();
        modBus.addListener((RegisterEvent e) -> register(e));
        modBus.addListener((FMLCommonSetupEvent e) -> e.enqueueWork(YogEffects::linkModifiers));
        modBus.addListener((AddPackFindersEvent e) -> e.addRepositorySource(new YogPackProvider(e.getPackType())));
        modBus.addListener((EntityAttributeCreationEvent e) -> YogEntityTypes.attributes(e::put));
        modBus.addListener((EntityAttributeModificationEvent e) -> YogEffects.playerAttributes().forEach(a -> e.add(EntityType.PLAYER, a)));

        hookServerEvents();
        if (FMLEnvironment.dist == Dist.CLIENT) YogClient.init(modBus);
    }

    /** A register event: the mods' steps for its registry. */
    private static void register(RegisterEvent event) {
        YogRegistrar.run(event.getRegistryKey(), new YogRegistrar.Sink() {
            @Override
            public <T> void register(ResourceKey<? extends Registry<T>> registry, ResourceLocation id, T value) {
                event.register(registry, id, () -> value);
            }
        });
    }

    private static void hookServerEvents() {
        var bus = MinecraftForge.EVENT_BUS;
        bus.addListener((ServerStartedEvent e) -> YogServerEvents.serverStarted(e.getServer()));
        bus.addListener((ServerStoppingEvent e) -> YogServerEvents.serverStopping());
        bus.addListener((TickEvent.ServerTickEvent e) -> {
            if (e.phase == TickEvent.Phase.END) YogServerEvents.serverTick(e.getServer());
        });
        bus.addListener((RegisterCommandsEvent e) -> YogCommands.register(e.getDispatcher()));
        bus.addListener((FurnaceFuelBurnTimeEvent e) -> {
            Integer ticks = YogContentRegistry.FUELS.get(e.getItemStack().getItem());
            if (ticks != null) e.setBurnTime(ticks);
        });

        bus.addListener((PlayerEvent.PlayerLoggedInEvent e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerJoined(sp);
        });
        bus.addListener((PlayerEvent.PlayerLoggedOutEvent e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerLeft(sp);
        });
        bus.addListener((PlayerEvent.PlayerRespawnEvent e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.respawned(sp, e.isEndConquered());
        });
        bus.addListener((PlayerEvent.PlayerChangedDimensionEvent e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.changedLevel(sp);
        });
        bus.addListener((PlayerEvent.StartTracking e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.startedTracking(e.getTarget(), sp);
        });

        bus.addListener((BlockEvent.BreakEvent e) -> {
            if (!(e.getLevel() instanceof Level level) || level.isClientSide()) return;
            if (!YogServerEvents.blockBreakPre(level, e.getPlayer(), e.getPos(), e.getState())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.blockBroken(level, e.getPlayer(), e.getPos(), e.getState(), level.getBlockEntity(e.getPos()));
        });
        bus.addListener((ServerChatEvent e) -> {
            if (!YogServerEvents.chatPre(e.getPlayer(), e.getRawText())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.chat(e.getPlayer(), e.getRawText());
        });

        bus.addListener((PlayerInteractEvent.RightClickItem e) -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useItem(sp, e.getHand())) e.setCanceled(true);
        });
        bus.addListener((PlayerInteractEvent.RightClickBlock e) -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useBlock(sp, e.getHand(), e.getHitVec())) e.setCanceled(true);
        });
        bus.addListener((PlayerInteractEvent.EntityInteract e) -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.interactEntity(sp, e.getHand(), e.getTarget())) e.setCanceled(true);
        });
        bus.addListener((AttackEntityEvent e) -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.attackEntity(sp, e.getTarget());
        });

        bus.addListener((LivingAttackEvent e) -> {
            if (!e.getEntity().level().isClientSide() && !YogServerEvents.damage(e.getEntity(), e.getSource(), e.getAmount())) e.setCanceled(true);
        });
        bus.addListener((LivingDeathEvent e) -> {
            LivingEntity entity = e.getEntity();
            if (entity.level().isClientSide()) return;
            if (entity instanceof ServerPlayer sp && !YogServerEvents.playerDeathPre(sp, e.getSource())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.died(entity, e.getSource());
        });
        bus.addListener((EntityJoinLevelEvent e) -> {
            if (!e.getLevel().isClientSide() && !YogServerEvents.entityLoaded(e.getEntity(), e.getLevel())) e.setCanceled(true);
        });
        bus.addListener((EntityLeaveLevelEvent e) -> {
            if (!e.getLevel().isClientSide()) YogServerEvents.entityUnloaded(e.getEntity());
        });
    }
}
