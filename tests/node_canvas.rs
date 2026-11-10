//! The node canvas wires an output to an input of its type, picks a wire up by its input,
//! deletes the selected node and reports the assistant's node kept.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::graph::{GraphNode, NodeCanvas, NodeCanvasEvent, NodeId, Wire};
use atelier::{Theme, WindowFrame};
use gpui::{
    AppContext, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window,
};

struct Host {
    canvas: Entity<NodeCanvas>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new().size_full().child(self.canvas.clone())
    }
}

type Log = Rc<RefCell<Vec<NodeCanvasEvent>>>;

fn open(cx: &mut TestAppContext) -> (Entity<NodeCanvas>, Log, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log: Log = Rc::default();
    let (host, window) = cx.add_window_view(|_, cx| {
        let canvas = cx.new(|cx| {
            let mut canvas = NodeCanvas::new("graph").port_types(["Audio", "Value"]);
            canvas.add(
                GraphNode::new("Source")
                    .at(0., 0.)
                    .output("Audio", 0)
                    .comment("Under the vocal\nChorus only"),
                cx,
            );
            canvas.add(
                GraphNode::new("Sink")
                    .at(320., 0.)
                    .input("In", 0)
                    .input("Gain", 1),
                cx,
            );
            canvas.add(
                GraphNode::new("Suggested")
                    .at(0., 200.)
                    .input("In", 0)
                    .by_agent(true),
                cx,
            );
            canvas
        });
        Host { canvas }
    });
    let canvas = host.read_with(window, |host, _| host.canvas.clone());
    window.update(|_, cx| {
        let log = log.clone();
        cx.subscribe(&canvas, move |_, event: &NodeCanvasEvent, _| {
            log.borrow_mut().push(event.clone())
        })
        .detach();
    });
    window.run_until_parked();
    (canvas, log, window)
}

fn center(selector: &'static str, cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no {selector}"))
        .center()
}

fn drag(from: &'static str, to: &'static str, cx: &mut VisualTestContext) {
    let (start, end) = (center(from, cx), center(to, cx));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn a_wire_joins_ports_of_one_type(cx: &mut TestAppContext) {
    let (canvas, log, cx) = open(cx);
    // Audio into Gain (a value) is refused; into In it holds.
    drag("graph-out-0-0", "graph-in-1-1", cx);
    drag("graph-out-0-0", "graph-in-1-0", cx);
    let wire = Wire::new(NodeId(0), 0, NodeId(1), 0);
    assert_eq!(*log.borrow(), [NodeCanvasEvent::Connected(wire)]);
    assert_eq!(
        canvas.read_with(cx, |canvas, _| canvas.wires().to_vec()),
        [wire]
    );

    // Picking it up by its input and letting go on the background drops it.
    drag("graph-in-1-0", "graph", cx);
    assert_eq!(
        log.borrow().last(),
        Some(&NodeCanvasEvent::Disconnected(wire))
    );
    assert!(canvas.read_with(cx, |canvas, _| canvas.wires().is_empty()));
}

#[gpui::test]
fn delete_removes_the_selected_node(cx: &mut TestAppContext) {
    let (canvas, log, cx) = open(cx);
    let title = center("graph-node-1", cx);
    cx.simulate_click(title, Modifiers::default());
    cx.run_until_parked();
    cx.simulate_keystrokes("delete");
    assert_eq!(
        *log.borrow(),
        [
            NodeCanvasEvent::Selected(Some(NodeId(1))),
            NodeCanvasEvent::Removed(NodeId(1))
        ]
    );
    assert!(canvas.read_with(cx, |canvas, _| canvas.node(NodeId(1)).is_none()));
}

#[gpui::test]
fn keeping_the_assistants_node_clears_its_mark(cx: &mut TestAppContext) {
    let (canvas, log, cx) = open(cx);
    let keep = center("graph-keep-2", cx);
    cx.simulate_click(keep, Modifiers::default());
    cx.run_until_parked();
    assert!(log.borrow().contains(&NodeCanvasEvent::Kept(NodeId(2))));
    assert!(canvas.read_with(cx, |canvas, _| !canvas.node(NodeId(2)).unwrap().by_agent));
}

#[gpui::test]
fn pressing_a_comment_asks_to_edit_it_and_does_not_move_the_node(cx: &mut TestAppContext) {
    let (canvas, log, cx) = open(cx);
    let before = canvas.read_with(cx, |canvas, _| canvas.node(NodeId(0)).unwrap().position);
    let note = center("graph-comment-0-0", cx);
    cx.simulate_mouse_down(note, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        note + gpui::point(gpui::px(40.), gpui::px(30.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(note, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        log.borrow().as_slice(),
        [
            NodeCanvasEvent::Selected(Some(NodeId(0))),
            NodeCanvasEvent::CommentPressed {
                node: NodeId(0),
                index: 0
            }
        ]
    );
    let after = canvas.read_with(cx, |canvas, _| canvas.node(NodeId(0)).unwrap().position);
    assert_eq!(before, after);
}
