//! Surfaces get the pointer: a press over one is captured, the drags and
//! the release follow it (in its local coordinates), hovering sends moves
//! and a leave.

use std::sync::{Arc, Mutex};

use yog_api::GfxContext;
use yog_api::ui::{Action, FlexDir, Pointer, Rect, Surface, Units, Widget, surface, widget};
use yog_kit::testing::TestHost;
use yog_kit::{Anchor, Cx, Parts, ScreenMeta, Update, ViewEvent, ViewModel};

#[derive(Default)]
struct Vm {
    clicks: u32,
}

impl ViewModel for Vm {}

struct View;

impl yog_kit::View for View {
    type Vm = Vm;
    const META: ScreenMeta = ScreenMeta {
        id: "test:surface",
        w: 200.0,
        h: 100.0,
        anchor: Anchor::Center,
        modal: true,
        pause: false,
    };

    fn build() -> (Self, Widget, Parts) {
        let root = widget::panel(FlexDir::Row)
            .w(Units::Percent(100.0))
            .h(Units::Percent(100.0))
            .child(
                widget::surface("test:pad")
                    .id("pad")
                    .w(Units::Px(100.0))
                    .h(Units::Px(100.0))
                    .padding(10.0, 10.0, 10.0, 10.0),
            )
            .child(
                widget::button("ok")
                    .id("ok")
                    .w(50.0)
                    .h(20.0)
                    .action(Action::new(0)),
            );
        (View, root, Parts::default())
    }

    fn update(&mut self, _: &mut Widget, _: &Vm, _: &mut Update) {}

    fn dispatch(&mut self, vm: &mut Vm, _: &mut Cx<'_, Vm>, ev: ViewEvent) {
        if let ViewEvent::Action(_) = ev {
            vm.clicks += 1;
        }
    }
}

#[derive(Default)]
struct Pad {
    log: Mutex<Vec<(Pointer, (f32, f32))>>,
}

impl Surface for Pad {
    fn paint(&self, _: &GfxContext, _: Rect) {}
    fn pointer(&self, ev: Pointer, size: (f32, f32)) -> bool {
        self.log.lock().unwrap().push((ev, size));
        true
    }
}

#[test]
fn surface_takes_the_pointer() {
    let pad = Arc::new(Pad::default());
    surface::register("test:pad", pad.clone());
    let mut t = TestHost::<View>::new(Vm::default());
    t.size(200.0, 100.0).open();
    let r = t.rect("pad").unwrap();
    let (x, y) = (r.x + 20.0, r.y + 30.0);

    assert!(t.drag(&[(x, y), (x + 5.0, y), (x + 300.0, y)]));
    let log: Vec<_> = pad.log.lock().unwrap().drain(..).collect();
    let size = (80.0, 80.0);
    assert_eq!(
        log,
        vec![
            (Pointer::Move { x: 10.0, y: 20.0 }, size),
            (Pointer::Down { x: 10.0, y: 20.0 }, size),
            (Pointer::Drag { x: 15.0, y: 20.0 }, size),
            // Still the surface's, outside its box.
            (Pointer::Drag { x: 310.0, y: 20.0 }, size),
            (Pointer::Up { x: 310.0, y: 20.0 }, size),
        ]
    );

    // Leaving it, then clicking elsewhere: a leave, and the button works.
    assert!(t.click_id("ok"));
    assert_eq!(
        pad.log.lock().unwrap().drain(..).next(),
        Some((Pointer::Leave, size))
    );
    assert_eq!(t.vm().clicks, 1);
    surface::unregister("test:pad");
}
