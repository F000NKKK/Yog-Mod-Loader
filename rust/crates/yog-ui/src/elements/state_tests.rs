//! Tests of actions, visibility and state styles.

use yog_gfx::Color;

use crate::layout::{self, FlexDir, hit_path, node_at};
use crate::widget::{self, Action, StateFlags, StylePatch, Units};

fn tree() -> widget::Widget {
    widget::panel(FlexDir::Column)
        .w(100.0)
        .h(100.0)
        .gap(2.0)
        .child(widget::label("hidden").visible(false))
        .child(
            widget::panel(FlexDir::Row)
                .w(Units::Percent(100.0))
                .h(20.0)
                .action(Action {
                    owner: 1,
                    slot: Action::SELECT,
                    index: 3,
                })
                .child(widget::label("row"))
                .child(widget::button("btn").w(20.0).h(10.0).action(Action::new(7))),
        )
}

#[test]
fn invisible_takes_no_space_and_paths_mirror_the_tree() {
    let node = layout::compute(&tree(), 400.0, 400.0);
    assert_eq!(node.children.len(), 2);
    assert!(!node.children[0].visible);
    assert_eq!(node.children[0].rect.h, 0.0);
    assert!(node.children[0].children.is_empty());
    // No gap for the collapsed label: the row starts at the top.
    assert_eq!(node.children[1].rect.y, 0.0);
    assert_eq!(
        node_at(&node, &[1, 1]).unwrap().action,
        Some(Action::new(7))
    );
    assert!(node_at(&node, &[5]).is_none());
}

#[test]
fn deeper_actions_win_and_disabled_is_not_hit() {
    let mut t = tree();
    let node = layout::compute(&t, 400.0, 400.0);
    let btn = node_at(&node, &[1, 1]).unwrap().rect;
    assert_eq!(hit_path(&node, btn.x + 1.0, btn.y + 1.0), Some(vec![1, 1]));
    // Elsewhere in the row: the row itself.
    assert_eq!(hit_path(&node, 90.0, 15.0), Some(vec![1]));
    assert_eq!(hit_path(&node, 90.0, 90.0), None);
    t.get_path_mut(&[1, 1]).unwrap().enabled = false;
    let node = layout::compute(&t, 400.0, 400.0);
    assert_eq!(hit_path(&node, btn.x + 1.0, btn.y + 1.0), Some(vec![1]));
    t.get_path_mut(&[1]).unwrap().visible = false;
    let node = layout::compute(&t, 400.0, 400.0);
    assert_eq!(hit_path(&node, 90.0, 15.0), None);
}

#[test]
fn state_styles_and_inheritance() {
    let mut row = widget::panel(FlexDir::Row)
        .bg(Color(1))
        .action(Action::new(0))
        .state_style(
            StateFlags::HOVER,
            StylePatch {
                bg: Some(Color(2)),
                ..Default::default()
            },
        )
        .state_style(
            StateFlags::SELECTED,
            StylePatch {
                bg: Some(Color(3)),
                ..Default::default()
            },
        )
        .child(widget::label("x").color(Color(10)).state_style(
            StateFlags::SELECTED,
            StylePatch {
                color: Some(Color(11)),
                ..Default::default()
            },
        ))
        .child(widget::button("b").action(Action::new(1)));
    assert_eq!(row.effective_style().bg, Color(1));
    row.set_state_inherited(StateFlags::HOVER, true);
    assert_eq!(row.effective_style().bg, Color(2));
    row.set_state_inherited(StateFlags::SELECTED, true);
    // Later rules win.
    assert_eq!(row.effective_style().bg, Color(3));
    assert_eq!(row.children[0].effective_style().color, Color(11));
    // Stops at the interactive button.
    assert!(row.children[1].state.is_empty());
    row.set_state_inherited(StateFlags::SELECTED | StateFlags::HOVER, false);
    assert_eq!(row.effective_style().bg, Color(1));
    assert_eq!(row.children[0].effective_style().color, Color(10));
}

#[test]
fn disabled_patch_follows_enabled() {
    let b = widget::button("b").color(Color(1)).state_style(
        StateFlags::DISABLED,
        StylePatch {
            color: Some(Color(2)),
            ..Default::default()
        },
    );
    assert_eq!(b.effective_style().color, Color(1));
    assert_eq!(b.enabled(false).effective_style().color, Color(2));
}
