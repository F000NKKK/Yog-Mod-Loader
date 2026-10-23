package dev.yog.server;

import com.mojang.brigadier.Command;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.context.CommandContext;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.EntityArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.network.chat.Component;

/** The mods' commands, built as Brigadier trees and run by the mods; the same on every loader. */
public final class YogCommands {
    private YogCommands() {}

    /** Every mod's command into {@code dispatcher} (the loader's command registration). */
    public static void register(CommandDispatcher<CommandSourceStack> dispatcher) {
        // Typed commands: build proper Brigadier argument chains. A single
        // command name can have MULTIPLE distinct schemas (subcommands with
        // different arg counts) — each is registered separately; Brigadier
        // merges repeated `dispatcher.register()` calls that share a root
        // literal name into one command-tree node, so this produces one
        // root node with a sibling branch per distinct arg shape, instead
        // of collapsing them all down to whichever schema was seen last.
        java.util.List<YogTypes.CommandSpec> declared = new YogServices.Command.Declared().call();
        for (YogTypes.CommandSpec d : typedSchemaOrder(declared)) {
            if (d.schema() != null) dispatcher.register(buildTypedCommand(d.name(), d.schema().trim().isEmpty() ? new String[0] : d.schema().trim().split("\\s+")));
        }

        // Plain commands: greedy-string arg (or no args).
        java.util.Set<String> plain = new java.util.LinkedHashSet<>();
        for (YogTypes.CommandSpec d : declared) if (d.schema() == null) plain.add(d.name());
        for (String name : plain) {
            dispatcher.register(Commands.literal(name)
                    .executes(ctx -> runCommand(name, java.util.List.of(), ctx))
                    .then(Commands.argument("args", StringArgumentType.greedyString())
                            .executes(ctx -> runCommand(
                                    name, java.util.List.of(StringArgumentType.getString(ctx, "args").trim().split("\\s+")), ctx))));
        }
    }

    /**
     * Build a Brigadier literal node with typed argument chain from a schema like
     * {@code ["int", "player", "blockpos"]}.  Each arg is named {@code arg_N}.
     * The resolved args go to Rust as a list.
     */
    private static LiteralArgumentBuilder<CommandSourceStack>
            buildTypedCommand(String name, String[] schema) {
        var root = Commands.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, java.util.List.of(), ctx));
            return root;
        }
        // Build argument chain from last to first, wrapping inner with outer.
        ArgumentBuilder<CommandSourceStack, ?> chain = buildLeaf(name, schema, schema.length - 1);
        for (int i = schema.length - 2; i >= 0; i--) {
            chain = buildArgNode(schema[i], argName(i, schema[i])).then(chain);
            // Also allow executing with fewer args (partial match not standard; just attach executes at leaf).
        }
        root.then(chain);
        return root;
    }

    /**
     * A typed argument's node name — its position and type, so schemas that
     * differ there get separate Brigadier branches instead of being merged
     * into the first one's (Brigadier merges same-named nodes).
     */
    private static String argName(int index, String type) {
        return "arg_" + index + "_" + type;
    }

    /** Typed commands with greedy `string` args last: Brigadier tries a command's branches in registration order, so word chains go first. */
    private static java.util.List<YogTypes.CommandSpec> typedSchemaOrder(java.util.List<YogTypes.CommandSpec> declared) {
        java.util.List<YogTypes.CommandSpec> out = new java.util.ArrayList<>(declared);
        out.sort(java.util.Comparator.comparing(d -> d.schema() != null && d.schema().contains("string")));
        return out;
    }

    private static RequiredArgumentBuilder<CommandSourceStack, ?> buildArgNode(String type, String argName) {
        return switch (type) {
            case "int"      -> Commands.argument(argName, IntegerArgumentType.integer());
            case "float"    -> Commands.argument(argName, FloatArgumentType.floatArg());
            case "word"     -> Commands.argument(argName, StringArgumentType.word());
            case "string"   -> Commands.argument(argName, StringArgumentType.greedyString());
            case "player"   -> Commands.argument(argName, EntityArgument.player());
            case "blockpos" -> Commands.argument(argName, BlockPosArgument.blockPos());
            default         -> Commands.argument(argName, StringArgumentType.word());
        };
    }

    private static ArgumentBuilder<CommandSourceStack, ?> buildLeaf(String cmdName, String[] schema, int idx) {
        String type    = schema[idx];
        String argName = argName(idx, type);
        return buildArgNode(type, argName).executes(ctx -> {
            java.util.List<String> args = new java.util.ArrayList<>();
            for (int i = 0; i <= idx; i++) args.add(resolveArg(schema[i], argName(i, schema[i]), ctx));
            return runCommand(cmdName, args, ctx);
        });
    }

    private static String resolveArg(String type, String argName, CommandContext<CommandSourceStack> ctx) {
        try {
            return switch (type) {
                case "int"      -> String.valueOf(IntegerArgumentType.getInteger(ctx, argName));
                case "float"    -> String.valueOf(FloatArgumentType.getFloat(ctx, argName));
                case "word", "string" -> StringArgumentType.getString(ctx, argName);
                case "player"   -> EntityArgument.getPlayer(ctx, argName).getName().getString();
                case "blockpos" -> {
                    net.minecraft.core.BlockPos pos = BlockPosArgument.getBlockPos(ctx, argName);
                    yield pos.getX() + "," + pos.getY() + "," + pos.getZ();
                }
                default -> StringArgumentType.getString(ctx, argName);
            };
        } catch (Exception e) {
            return "";
        }
    }

    private static int runCommand(String name, java.util.List<String> args, CommandContext<CommandSourceStack> ctx) {
        CommandSourceStack src = ctx.getSource();
        net.minecraft.world.entity.Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getStringUUID() : "";
        String dim = src.getLevel().dimension().identifier().toString();
        String reply = new YogEvents.Server.Command.Run(name, args, src.getTextName(), uuid, dim, null).emitChanged().reply();
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }

}
