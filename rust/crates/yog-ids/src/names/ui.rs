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

    /// Where a `<Screen>` sits in the game's window (`anchor="top_left"`).
    screen_anchor {
        CENTER = crate::word::CENTER,
        TOP_LEFT = crate::concatcp!(crate::word::TOP, crate::sep::WORD, crate::word::LEFT),
        TOP = crate::word::TOP,
        TOP_RIGHT = crate::concatcp!(crate::word::TOP, crate::sep::WORD, crate::word::RIGHT),
        LEFT = crate::word::LEFT,
        RIGHT = crate::word::RIGHT,
        BOTTOM_LEFT = crate::concatcp!(crate::word::BOTTOM, crate::sep::WORD, crate::word::LEFT),
        BOTTOM = crate::word::BOTTOM,
        BOTTOM_RIGHT = crate::concatcp!(crate::word::BOTTOM, crate::sep::WORD, crate::word::RIGHT),
        /// The whole window.
        FILL = crate::word::FILL,
    }

    /// A view-model screen's markup: its item templates, the attributes of
    /// its lists and commands, a binding's options.
    yui_vm {
        SCREEN_RESOURCES = crate::concatcp!(crate::yui::SCREEN, crate::yui_markup::RESOURCES),
        DATA_TEMPLATE = "DataTemplate",
        /// A list's property elements: `<ListView.ItemTemplate>`.
        ITEM_TEMPLATE = "ItemTemplate",
        CONTENT_TEMPLATE = "ContentTemplate",
        EMPTY = "Empty",
        COMMAND = "command",
        ITEMS = crate::word::ITEMS,
        CONTENT = "content",
        KEY = "key",
        HORIZONTAL = "horizontal",
        /// A binding's options (`{x:Bind name, Mode=TwoWay}`) and modes.
        MODE = "Mode",
        BIND_BACK = "BindBack",
        FALLBACK = "Fallback",
        ONE_WAY = "OneWay",
        ONE_TIME = "OneTime",
        TWO_WAY = "TwoWay",
    }

    /// The states an attribute can be prefixed with: `hover:bg="#FF333333"`.
    widget_state {
        HOVER = "hover",
        PRESSED = "pressed",
        FOCUSED = "focused",
        SELECTED = crate::yui_attr::SELECTED,
        CHECKED = crate::yui_attr::CHECKED,
        DISABLED = "disabled",
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
