package dev.yog.nanite;

import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.bus.YogTypes;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * The game's names as nanites write them (Mojang's) → this host's run-time
 * names ({@code docs/design/nanites.md}, "Names"), from the table the host
 * jar carries ({@code yog mappings}); none on a host running Mojang's names,
 * where every name is itself. Classes in internal form
 * ({@code net/minecraft/world/entity/Entity}); a member is looked up on its
 * class, then up its supertypes (a method a subclass inherits).
 */
public final class YogMappings {
    private YogMappings() {}

    /** What joins an owner and a member in the keys. */
    private static final char SEP = ' ';

    private static final Map<String, String> CLASSES = new HashMap<>();
    /** Run-time class (internal) → Mojang's. */
    private static final Map<String, String> NAMED = new HashMap<>();
    /** {@code owner field} → run-time name. */
    private static final Map<String, String> FIELDS = new HashMap<>();
    /** {@code owner name desc} → run-time name. */
    private static final Map<String, String> METHODS = new HashMap<>();
    private static volatile boolean loaded;

    private static synchronized void load() {
        if (loaded) return;
        try (InputStream in = YogMappings.class.getResourceAsStream("/" + YogNames.MappingFile.NAME)) {
            if (in != null) read(YogTypes.Mappings.fromBytes(in.readAllBytes()));
        } catch (IOException | RuntimeException e) {
            YogLog.error("the mapping table can't be read: nanites see Mojang's names only", e);
        }
        loaded = true;
    }

    private static void read(YogTypes.Mappings table) {
        if (mojangAlready(table)) {
            YogLog.info("nanites: the game runs on Mojang's names here (a dev run): no mapping table");
            return;
        }
        for (YogTypes.ClassMap c : table.classes()) {
            CLASSES.put(c.named(), c.runtime());
            NAMED.put(c.runtime(), c.named());
            for (YogTypes.MemberMap f : c.fields()) FIELDS.put(c.named() + SEP + f.named(), f.runtime());
            for (YogTypes.MemberMap m : c.methods()) {
                METHODS.put(c.named() + SEP + m.named() + SEP + m.desc(), m.runtime());
            }
        }
    }

    /**
     * Whether the game's classes carry Mojang's names already (a dev run:
     * Loom and ForgeGradle remap the game for development) — a class the
     * table renames is there by its Mojang name.
     */
    private static boolean mojangAlready(YogTypes.Mappings table) {
        ClassLoader loader = YogMappings.class.getClassLoader();
        if (loader == null) return false;
        for (YogTypes.ClassMap c : table.classes()) {
            if (!c.named().equals(c.runtime())) return loader.getResource(c.named() + ".class") != null;
            // Forge's names keep the classes', renaming members only: a renamed
            // method is there by its Mojang name, or not.
            if (!c.methods().isEmpty()) return declares(loader, c.named(), c.methods().get(0).named());
        }
        return false;
    }

    /** Whether class {@code owner}'s bytes declare a method named {@code name} (not loading it). */
    private static boolean declares(ClassLoader loader, String owner, String name) {
        try (InputStream in = loader.getResourceAsStream(owner + ".class")) {
            if (in == null) return false;
            boolean[] found = {false};
            new org.objectweb.asm.ClassReader(in).accept(new org.objectweb.asm.ClassVisitor(org.objectweb.asm.Opcodes.ASM9) {
                @Override
                public org.objectweb.asm.MethodVisitor visitMethod(int access, String n, String d, String sig, String[] ex) {
                    if (n.equals(name)) found[0] = true;
                    return null;
                }
            }, org.objectweb.asm.ClassReader.SKIP_CODE);
            return found[0];
        } catch (IOException e) {
            return false;
        }
    }

    private static void ensure() {
        if (!loaded) load();
    }

    /** Class {@code named}'s run-time internal name. */
    public static String className(String named) {
        ensure();
        return CLASSES.getOrDefault(named, named);
    }

    /** Run-time class {@code runtime}'s Mojang name. */
    public static String namedOf(String runtime) {
        ensure();
        return NAMED.getOrDefault(runtime, runtime);
    }

    /** Class {@code named}, loaded. */
    public static Class<?> load(String named) throws ClassNotFoundException {
        return Class.forName(className(named).replace('/', '.'), false, YogMappings.class.getClassLoader());
    }

    /** Descriptor {@code desc} (Mojang's names) in run-time names. */
    public static String desc(String desc) {
        ensure();
        if (CLASSES.isEmpty()) return desc;
        StringBuilder out = new StringBuilder(desc.length());
        int at = 0;
        while (at < desc.length()) {
            char c = desc.charAt(at);
            out.append(c);
            if (c == 'L') {
                int end = desc.indexOf(';', at);
                out.append(className(desc.substring(at + 1, end))).append(';');
                at = end;
            }
            at++;
        }
        return out.toString();
    }

    /** Method {@code name desc} declared by class {@code owner} (Mojang's): its run-time name, no class loaded. */
    public static String declaredMethod(String owner, String name, String desc) {
        ensure();
        return METHODS.getOrDefault(owner + SEP + name + SEP + desc, name);
    }

    /** Field {@code name} declared by class {@code owner} (Mojang's): its run-time name, no class loaded. */
    public static String declaredField(String owner, String name) {
        ensure();
        return FIELDS.getOrDefault(owner + SEP + name, name);
    }

    /** Field {@code name} of {@code owner} or a supertype: its run-time name. */
    public static String field(Class<?> owner, String name) {
        return member(owner, name, null, FIELDS);
    }

    /** Method {@code name desc} of {@code owner} or a supertype: its run-time name. */
    public static String method(Class<?> owner, String name, String desc) {
        return member(owner, name, desc, METHODS);
    }

    private static String member(Class<?> owner, String name, String desc, Map<String, String> table) {
        ensure();
        if (table.isEmpty()) return name;
        Deque<Class<?>> todo = new ArrayDeque<>(List.of(owner));
        while (!todo.isEmpty()) {
            Class<?> c = todo.poll();
            String key = namedOf(c.getName().replace('.', '/')) + SEP + name + (desc == null ? "" : SEP + desc);
            String found = table.get(key);
            if (found != null) return found;
            if (c.getSuperclass() != null) todo.add(c.getSuperclass());
            todo.addAll(List.of(c.getInterfaces()));
        }
        return name;
    }
}
