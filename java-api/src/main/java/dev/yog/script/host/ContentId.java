package dev.yog.script.host;

import dev.yog.YogNames;

/**
 * {@code ContentId<TSelf>}'s members ({@code ItemId}, {@code BlockId}, …):
 * a static each, the id first. An id is its text on the JVM.
 */
public final class ContentId {
    private ContentId() {}

    /** The id {@code text} names: one without a namespace is the game's. */
    public static java.lang.String Of(java.lang.String text) {
        return text.contains(YogNames.Sep.NAMESPACE) ? text : YogNames.Ns.MINECRAFT + YogNames.Sep.NAMESPACE + text;
    }

    public static java.lang.String Id(java.lang.String id) {
        return id;
    }

    public static java.lang.String Namespace(java.lang.String id) {
        return id.substring(0, id.indexOf(YogNames.Sep.NAMESPACE));
    }

    public static java.lang.String Path(java.lang.String id) {
        return id.substring(id.indexOf(YogNames.Sep.NAMESPACE) + YogNames.Sep.NAMESPACE.length());
    }
}
