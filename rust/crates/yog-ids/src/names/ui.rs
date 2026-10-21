//! Component `.yui` files: what their declarations are written with.

names! {
    /// A `<Component>` root's designer attributes, and a `<Property>`'s /
    /// `<Event>`'s.
    yui_component {
        NAME = crate::word::NAME,
        DISPLAY_NAME = "display_name",
        DESCRIPTION = "description",
        ICON = "icon",
        CATEGORY = "category",
        DESIGN_W = "design_w",
        DESIGN_H = "design_h",
        TOOLBOX = "toolbox",
        TYPE = crate::word::TYPE,
        DEFAULT = crate::word::DEFAULT,
        OPTIONS = "options",
        DOC = "doc",
        /// What starts a design-time attribute: the designer's sample values.
        DESIGN_PREFIX = "d:",
    }

    /// The types of a component's properties.
    prop_kind {
        TEXT = crate::word::TEXT,
        NUMBER = crate::word::NUMBER,
        INT = "int",
        BOOL = crate::word::BOOL,
        COLOR = crate::word::COLOR,
        CHOICE = "choice",
        EVENT = "event",
        IMAGE = crate::word::IMAGE,
    }
}
