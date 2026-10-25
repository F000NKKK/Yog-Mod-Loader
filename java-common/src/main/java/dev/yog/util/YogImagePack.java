package dev.yog.util;

import dev.yog.YogNames;
import dev.yog.bus.YogServices;
import java.io.BufferedInputStream;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.function.BiConsumer;

/**
 * The staged pack's resources with the {@code .yog}s' yimages handed to the
 * game as PNG, made in memory ({@code yog_media::yimage}): a yimage keeps
 * its {@code .png} name, so the game finds and lists it as ever, and no PNG
 * is on disk.
 *
 * <p>The game's pack types are each version's own and named otherwise on
 * some hosts, so they're wrapped by shape, not by name: what a method
 * returns that is the game's (a resource supplier) and what it's handed
 * that takes resources (the output of a listing) are wrapped; a stream
 * either opens that is a yimage is read as its PNG.
 */
public final class YogImagePack {
    private YogImagePack() {}

    private static final byte[] MAGIC = YogNames.Yimage.MAGIC.getBytes(StandardCharsets.US_ASCII);
    /** The package of what isn't the game's (collections, strings, streams). */
    private static final String JAVA = "java.";

    /** {@code resources}, a {@code type} (the game's pack resources), wrapped. */
    public static <T> T wrap(T resources, Class<T> type) {
        return type.cast(proxy(type, resources));
    }

    private static Object proxy(Class<?> type, Object target) {
        return Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type}, (p, m, args) -> call(target, m, args));
    }

    private static Object call(Object target, Method m, Object[] args) throws Throwable {
        Class<?>[] params = m.getParameterTypes();
        if (args != null) {
            for (int i = 0; i < args.length; i++) args[i] = wrapArg(params[i], args[i]);
        }
        Object out;
        try {
            out = m.invoke(target, args);
        } catch (InvocationTargetException e) {
            throw e.getCause();
        }
        if (out instanceof InputStream in) return png(in);
        return game(m.getReturnType()) && out != null ? proxy(m.getReturnType(), out) : out;
    }

    /** What takes resources (a listing's output) wrapped: each supplier it's handed is. */
    private static Object wrapArg(Class<?> type, Object arg) {
        if (arg == null || !type.isInterface() || !BiConsumer.class.isAssignableFrom(type)) return arg;
        return Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type}, (p, m, args) -> {
            if (args != null && args.length == 2 && args[1] != null && m.getName().equals("accept")) {
                Class<?> supplier = args[1].getClass().getInterfaces().length > 0 ? args[1].getClass().getInterfaces()[0] : null;
                if (supplier != null && game(supplier)) args[1] = proxy(supplier, args[1]);
            }
            try {
                return m.invoke(arg, args);
            } catch (InvocationTargetException e) {
                throw e.getCause();
            }
        });
    }

    /** Whether {@code type} is one of the game's interfaces (not Java's). */
    private static boolean game(Class<?> type) {
        return type.isInterface() && !type.getName().startsWith(JAVA);
    }

    /** {@code in}, or — a yimage — its PNG. */
    public static InputStream png(InputStream in) throws IOException {
        BufferedInputStream b = new BufferedInputStream(in);
        b.mark(MAGIC.length);
        byte[] head = b.readNBytes(MAGIC.length);
        b.reset();
        if (!Arrays.equals(head, MAGIC)) return b;
        byte[] yimage;
        try (b) {
            yimage = b.readAllBytes();
        }
        return new ByteArrayInputStream(new YogServices.Media.Png(yimage).call().png());
    }
}
