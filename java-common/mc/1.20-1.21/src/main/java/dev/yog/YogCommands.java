package dev.yog;

import com.mojang.brigadier.Command;
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
        String schemaLines = NativeBridge.nativeTypedCommandSchemas();
        if (schemaLines != null) {
            for (String line : typedSchemaOrder(schemaLines)) {
                if (line.isBlank()) continue;
                int tab = line.indexOf('\t');
                if (tab <= 0) continue;
                String name   = line.substring(0, tab);
                String schema = line.substring(tab + 1);
                dispatcher.register(buildTypedCommand(name, schema.split("\\s+")));
            }
        }

        // Plain commands: greedy-string arg (or no args).
        String names = NativeBridge.nativeCommandNames();
        if (names == null || names.isBlank()) return;
        for (String name : names.split("\n")) {
            if (name.isBlank()) continue;
            dispatcher.register(Commands.literal(name)
                    .executes(ctx -> runCommand(name, "", ctx))
                    .then(Commands.argument("args", StringArgumentType.greedyString())
                            .executes(ctx -> runCommand(
                                    name, StringArgumentType.getString(ctx, "args").trim().replaceAll("\\s+", "\t"), ctx))));
        }
    }

    /**
     * Build a Brigadier literal node with typed argument chain from a schema like
     * {@code ["int", "player", "blockpos"]}.  Each arg is named {@code arg_N}.
     * All resolved args are serialised tab-separated and forwarded to Rust.
     */
    private static LiteralArgumentBuilder<CommandSourceStack>
            buildTypedCommand(String name, String[] schema) {
        var root = Commands.literal(name);
        if (schema.length == 0) {
            root.executes(ctx -> runCommand(name, "", ctx));
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

    /** Schema lines with greedy `string` args last: Brigadier tries a command's branches in registration order, so word chains go first. */
    private static java.util.List<String> typedSchemaOrder(String lines) {
        java.util.List<String> out = new java.util.ArrayList<>(java.util.Arrays.asList(lines.split("\n")));
        out.sort(java.util.Comparator.comparing(l -> l.contains("string")));
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
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i <= idx; i++) {
                if (i > 0) sb.append('\t');
                sb.append(resolveArg(schema[i], argName(i, schema[i]), ctx));
            }
            return runCommand(cmdName, sb.toString(), ctx);
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

    private static int runCommand(String name, String args, CommandContext<CommandSourceStack> ctx) {
        CommandSourceStack src = ctx.getSource();
        net.minecraft.world.entity.Entity entity = src.getEntity();
        String uuid = entity != null ? entity.getStringUUID() : "";
        String dim = src.getLevel().dimension().location().toString();
        String reply = NativeBridge.nativeOnCommand(name, args, src.getTextName(), uuid, dim);
        if (reply != null && !reply.isEmpty()) {
            src.sendSuccess(() -> Component.literal(reply), false);
        }
        return Command.SINGLE_SUCCESS;
    }

}
