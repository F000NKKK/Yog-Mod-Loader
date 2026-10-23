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
import net.minecraft.world.entity.LivingEntity;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.loading.FMLEnvironment;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.AddPackFindersEvent;
import net.neoforged.neoforge.event.RegisterCommandsEvent;
import net.neoforged.neoforge.event.ServerChatEvent;
import net.neoforged.neoforge.event.brewing.RegisterBrewingRecipesEvent;
import net.neoforged.neoforge.event.entity.EntityAttributeCreationEvent;
import net.neoforged.neoforge.event.entity.EntityAttributeModificationEvent;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;
import net.neoforged.neoforge.event.entity.EntityLeaveLevelEvent;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;
import net.neoforged.neoforge.event.entity.living.LivingIncomingDamageEvent;
import net.neoforged.neoforge.event.entity.player.AttackEntityEvent;
import net.neoforged.neoforge.event.entity.player.PlayerEvent;
import net.neoforged.neoforge.event.entity.player.PlayerInteractEvent;
import net.neoforged.neoforge.event.furnace.FurnaceFuelBurnTimeEvent;
import net.neoforged.neoforge.event.level.block.BreakBlockEvent;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.server.ServerStoppingEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;
import net.neoforged.neoforge.network.event.RegisterPayloadHandlersEvent;
import net.neoforged.neoforge.registries.RegisterEvent;

/**
 * NeoForge's entry point: boots the runtime, registers the mods' content
 * ({@link YogContentRegistry}) registry by registry, and hooks NeoForge's
 * events up to {@link YogServerEvents}. All it knows is NeoForge.
 */
@Mod(YogNames.Ns.YOG)
public class YogHost {
    /** The version of Yog's network protocol (its one payload). */
    private static final String PROTOCOL = "1";

    public YogHost(IEventBus modBus) {
        NativeBridge.ensureLoaded();
        YogLog.info("NeoForge host initialised.");
        YogContentRegistry.schedule();

        modBus.addListener(RegisterEvent.class, YogHost::register);
        modBus.addListener(RegisterPayloadHandlersEvent.class, e -> e.registrar(PROTOCOL).playBidirectional(YogPayload.TYPE, YogPayload.CODEC,
                (payload, ctx) -> ctx.enqueueWork(payload::toClient),
                (payload, ctx) -> {
                    if (ctx.player() instanceof ServerPlayer sp) ctx.enqueueWork(() -> payload.toServer(sp));
                }));
        modBus.addListener(AddPackFindersEvent.class, e -> e.addRepositorySource(new YogPackProvider(e.getPackType())));
        modBus.addListener(EntityAttributeCreationEvent.class, e -> YogEntityTypes.attributes((type, a) -> e.put(type, a.build())));
        modBus.addListener(EntityAttributeModificationEvent.class,
                e -> YogEffects.PLAYER_ATTRIBUTES.forEach(a -> e.add(net.minecraft.world.entity.EntityTypes.PLAYER, a)));

        hookServerEvents();
        if (FMLEnvironment.getDist() == Dist.CLIENT) YogClient.init(modBus);
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
        var bus = NeoForge.EVENT_BUS;
        bus.addListener(ServerStartedEvent.class, e -> YogServerEvents.serverStarted(e.getServer()));
        bus.addListener(ServerStoppingEvent.class, e -> YogServerEvents.serverStopping());
        bus.addListener(ServerTickEvent.Post.class, e -> YogServerEvents.serverTick(e.getServer()));
        bus.addListener(RegisterCommandsEvent.class, e -> YogCommands.register(e.getDispatcher()));
        bus.addListener(FurnaceFuelBurnTimeEvent.class, e -> {
            Integer ticks = YogContentRegistry.FUELS.get(e.getItemStack().getItem());
            if (ticks != null) e.setBurnTime(ticks);
        });
        bus.addListener(RegisterBrewingRecipesEvent.class, e -> YogEffects.mixes(e.getBuilder()::addMix));

        bus.addListener(PlayerEvent.PlayerLoggedInEvent.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerJoined(sp);
        });
        bus.addListener(PlayerEvent.PlayerLoggedOutEvent.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.playerLeft(sp);
        });
        bus.addListener(PlayerEvent.PlayerRespawnEvent.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.respawned(sp, e.isEndConquered());
        });
        bus.addListener(PlayerEvent.PlayerChangedDimensionEvent.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.changedLevel(sp);
        });
        bus.addListener(PlayerEvent.StartTracking.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.startedTracking(e.getTarget(), sp);
        });

        bus.addListener(BreakBlockEvent.class, e -> {
            if (e.getLevel().isClientSide()) return;
            var level = (net.minecraft.world.level.Level) e.getLevel();
            if (!YogServerEvents.blockBreakPre(level, e.getPlayer(), e.getPos(), e.getState())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.blockBroken(level, e.getPlayer(), e.getPos(), e.getState(), level.getBlockEntity(e.getPos()));
        });
        bus.addListener(ServerChatEvent.class, e -> {
            if (!YogServerEvents.chatPre(e.getPlayer(), e.getRawText())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.chat(e.getPlayer(), e.getRawText());
        });

        bus.addListener(PlayerInteractEvent.RightClickItem.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useItem(sp, e.getHand())) e.setCanceled(true);
        });
        bus.addListener(PlayerInteractEvent.RightClickBlock.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.useBlock(sp, e.getHand(), e.getHitVec())) e.setCanceled(true);
        });
        bus.addListener(PlayerInteractEvent.EntityInteract.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp && !YogServerEvents.interactEntity(sp, e.getHand(), e.getTarget())) e.setCanceled(true);
        });
        bus.addListener(AttackEntityEvent.class, e -> {
            if (e.getEntity() instanceof ServerPlayer sp) YogServerEvents.attackEntity(sp, e.getTarget());
        });

        bus.addListener(LivingIncomingDamageEvent.class, e -> {
            if (!e.getEntity().level().isClientSide() && !YogServerEvents.damage(e.getEntity(), e.getSource(), e.getAmount())) e.setCanceled(true);
        });
        bus.addListener(LivingDeathEvent.class, e -> {
            LivingEntity entity = e.getEntity();
            if (entity.level().isClientSide()) return;
            if (entity instanceof ServerPlayer sp && !YogServerEvents.playerDeathPre(sp, e.getSource())) {
                e.setCanceled(true);
                return;
            }
            YogServerEvents.died(entity, e.getSource());
        });
        bus.addListener(EntityJoinLevelEvent.class, e -> {
            if (!e.getLevel().isClientSide() && !YogServerEvents.entityLoaded(e.getEntity(), e.getLevel())) e.setCanceled(true);
        });
        bus.addListener(EntityLeaveLevelEvent.class, e -> {
            if (!e.getLevel().isClientSide()) YogServerEvents.entityUnloaded(e.getEntity());
        });
    }
}
