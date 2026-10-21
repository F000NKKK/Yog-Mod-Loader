package dev.yog;

import dev.yog.data.YogContent;

import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.biome.v1.BiomeModifications;
import net.fabricmc.fabric.api.biome.v1.BiomeSelectors;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.entity.event.v1.ServerEntityLevelChangeEvents;
import net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents;
import net.fabricmc.fabric.api.entity.event.v1.ServerPlayerEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerEntityEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.event.player.AttackEntityCallback;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseBlockCallback;
import net.fabricmc.fabric.api.event.player.UseEntityCallback;
import net.fabricmc.fabric.api.event.player.UseItemCallback;
import net.fabricmc.fabric.api.message.v1.ServerMessageEvents;
import net.fabricmc.fabric.api.networking.v1.EntityTrackingEvents;
import net.fabricmc.fabric.api.networking.v1.PayloadTypeRegistry;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.fabricmc.fabric.api.registry.FabricPotionBrewingBuilder;
import net.fabricmc.fabric.api.registry.FuelValueEvents;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.tags.TagKey;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.levelgen.GenerationStep;

/**
 * Fabric's entry point: boots the runtime, registers the mods' content
 * ({@link YogContentRegistry}) and hooks Fabric's events up to
 * {@link YogServerEvents}. All it knows is Fabric.
 */
public class YogHost implements ModInitializer {
    @Override
    public void onInitialize() {
        NativeBridge.ensureLoaded();
        YogLog.info("Fabric host initialised.");

        YogContentRegistry.schedule();
        YogRegistrar.runAll(YogHost::register);
        addBiomeFeatures();
        FuelValueEvents.BUILD.register((fuels, context) -> YogContentRegistry.FUELS.forEach(fuels::add));
        FabricPotionBrewingBuilder.BUILD.register(builder -> YogEffects.mixes(builder::addMix));
        YogEntityTypes.attributes(FabricDefaultAttributeRegistry::register);
        if (!YogEffects.PLAYER_ATTRIBUTES.isEmpty()) {
            AttributeSupplier.Builder player = Player.createAttributes();
            YogEffects.PLAYER_ATTRIBUTES.forEach(player::add);
            FabricDefaultAttributeRegistry.register(EntityTypes.PLAYER, player);
        }

        PayloadTypeRegistry.clientboundPlay().register(YogPayload.TYPE, YogPayload.CODEC);
        PayloadTypeRegistry.serverboundPlay().register(YogPayload.TYPE, YogPayload.CODEC);
        ServerPlayNetworking.registerGlobalReceiver(YogPayload.TYPE, (payload, context) -> {
            ServerPlayer player = context.player();
            player.level().getServer().execute(() -> payload.toServer(player));
        });

        hookServerEvents();
        CommandRegistrationCallback.EVENT.register((dispatcher, registryAccess, environment) -> YogCommands.register(dispatcher));
    }

    private static <T> void register(ResourceKey<? extends Registry<T>> registry, Identifier id, T value) {
        @SuppressWarnings("unchecked") // a registry of the key's type: the root registry hands it out untyped
        Registry<T> r = (Registry<T>) BuiltInRegistries.REGISTRY.getValue(registry.identifier());
        Registry.register(r, id, value);
    }

    /** The mods' features in biomes (their ores): Fabric adds them in code; Forge and NeoForge read the biome modifiers the build wrote. */
    private static void addBiomeFeatures() {
        for (YogContent.BiomeFeature f : YogContent.cached(NativeBridge::nativeContent).biomeFeatures) {
            Identifier feature = Identifier.tryParse(f.feature()), biomes = Identifier.tryParse(f.biomes());
            if (feature == null || biomes == null) continue;
            GenerationStep.Decoration step;
            try {
                step = GenerationStep.Decoration.valueOf(f.step().toUpperCase(java.util.Locale.ROOT));
            } catch (IllegalArgumentException e) {
                YogLog.warn(f.feature() + ": no generation step " + f.step());
                continue;
            }
            BiomeModifications.addFeature(BiomeSelectors.tag(TagKey.create(Registries.BIOME, biomes)), step,
                    ResourceKey.create(Registries.PLACED_FEATURE, feature));
        }
    }

    private static InteractionResult allow(boolean allowed) {
        return allowed ? InteractionResult.PASS : InteractionResult.FAIL;
    }

    private static void hookServerEvents() {
        ServerLifecycleEvents.SERVER_STARTED.register(YogServerEvents::serverStarted);
        ServerLifecycleEvents.SERVER_STOPPING.register(server -> YogServerEvents.serverStopping());
        ServerTickEvents.END_SERVER_TICK.register(YogServerEvents::serverTick);

        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> YogServerEvents.playerJoined(handler.player));
        ServerPlayConnectionEvents.DISCONNECT.register((handler, server) -> YogServerEvents.playerLeft(handler.player));
        ServerPlayerEvents.AFTER_RESPAWN.register((oldPlayer, newPlayer, alive) -> YogServerEvents.respawned(newPlayer, alive));
        ServerEntityLevelChangeEvents.AFTER_PLAYER_CHANGE_LEVEL.register((player, origin, destination) -> YogServerEvents.changedLevel(player));
        EntityTrackingEvents.START_TRACKING.register(YogServerEvents::startedTracking);

        PlayerBlockBreakEvents.BEFORE.register((level, player, pos, state, be) -> YogServerEvents.blockBreakPre(level, player, pos, state));
        PlayerBlockBreakEvents.AFTER.register(YogServerEvents::blockBroken);

        ServerMessageEvents.ALLOW_CHAT_MESSAGE.register((message, sender, params) ->
                YogServerEvents.chatPre(sender, message.decoratedContent().getString()));
        ServerMessageEvents.CHAT_MESSAGE.register((message, sender, params) ->
                YogServerEvents.chat(sender, message.decoratedContent().getString()));

        UseItemCallback.EVENT.register((player, level, hand) ->
                !level.isClientSide() && player instanceof ServerPlayer sp ? allow(YogServerEvents.useItem(sp, hand)) : InteractionResult.PASS);
        UseBlockCallback.EVENT.register((player, level, hand, hit) ->
                !level.isClientSide() && player instanceof ServerPlayer sp ? allow(YogServerEvents.useBlock(sp, hand, hit)) : InteractionResult.PASS);
        UseEntityCallback.EVENT.register((player, level, hand, entity, hit) ->
                !level.isClientSide() && player instanceof ServerPlayer sp ? allow(YogServerEvents.interactEntity(sp, hand, entity)) : InteractionResult.PASS);
        AttackEntityCallback.EVENT.register((player, level, hand, entity, hit) -> {
            if (!level.isClientSide() && player instanceof ServerPlayer sp) YogServerEvents.attackEntity(sp, entity);
            return InteractionResult.PASS;
        });

        ServerLivingEntityEvents.ALLOW_DAMAGE.register(YogServerEvents::damage);
        ServerLivingEntityEvents.AFTER_DEATH.register(YogServerEvents::died);
        ServerLivingEntityEvents.ALLOW_DEATH.register((entity, source, amount) ->
                !(entity instanceof ServerPlayer sp) || YogServerEvents.playerDeathPre(sp, source));
        ServerEntityEvents.ENTITY_LOAD.register((entity, level) -> {
            if (!YogServerEvents.entityLoaded(entity, level)) entity.discard();
        });
        ServerEntityEvents.ENTITY_UNLOAD.register((entity, level) -> YogServerEvents.entityUnloaded(entity));
    }
}
