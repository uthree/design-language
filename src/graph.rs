//! A node graph on a pannable canvas: nodes with typed ports, wires between them, and the
//! pointer handling that moves, wires and pans them.
//!
//! [`NodeCanvas`] is a view the application holds, like [`crate::agent::ChatView`]: it keeps
//! the nodes, the wires, the selection and the pan, and reports what the person did as
//! [`NodeCanvasEvent`]s. What a node *means* stays in the application: give each node its
//! title, ports and [`NodeField`] rows, and update its [`NodeState`] while a run goes.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, ElementId, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, PathBuilder, Pixels, Point, Render, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, fill, point, prelude::FluentBuilder,
    px, relative, size,
};

use crate::button::{Button, ControlSize};
use crate::display::{Divider, Spinner, StatusDot};
use crate::focus::selector;
use crate::icon::{Icon, IconName};
use crate::overlay::Tooltip;
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

/// A node on a [`NodeCanvas`], as [`NodeCanvas::add`] returned it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

/// One input or output of a node: its name and the type of value it carries.
///
/// The type is an index into [`NodeCanvas::port_types`] and picks its colour from
/// [`crate::Colors::ports`]; wires only join ports of the same type.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// What the port is, shown beside its socket.
    pub name: SharedString,
    /// The type of value it carries.
    pub kind: usize,
}

impl Port {
    /// A port named `name` carrying values of type `kind`.
    pub fn new(name: impl Into<SharedString>, kind: usize) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}

/// A wire from one node's output to another node's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Wire {
    /// The node the value comes from.
    pub from: NodeId,
    /// Which of its outputs.
    pub output: usize,
    /// The node the value goes to.
    pub to: NodeId,
    /// Which of its inputs.
    pub input: usize,
}

impl Wire {
    /// A wire from `from`'s output `output` to `to`'s input `input`.
    pub fn new(from: NodeId, output: usize, to: NodeId, input: usize) -> Self {
        Self {
            from,
            output,
            to,
            input,
        }
    }
}

/// Where a node is in a run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NodeState {
    /// Not part of a run.
    #[default]
    Idle,
    /// Waiting for its turn.
    Queued,
    /// Running, with progress from 0 to 1.
    Running(f32),
    /// Finished, with how long it took in seconds if known.
    Done(Option<f32>),
    /// Nothing upstream changed, so its last result was reused.
    Cached,
    /// Stopped by an error.
    Failed,
}

/// A row in a node's body: a parameter at a glance. Edit it in an inspector beside the
/// canvas, and put the new value back with [`NodeCanvas::update_node`].
#[derive(Clone, Debug, PartialEq)]
pub enum NodeField {
    /// A number, drawn as a well filled to `fraction` (0–1) with its value at the end.
    Number {
        /// What it sets.
        name: SharedString,
        /// The value as the person reads it, with its unit: "120 BPM".
        value: SharedString,
        /// Where the value sits in its range, from 0 to 1.
        fraction: f32,
    },
    /// One of a few options, with a chevron.
    Choice {
        /// What it sets.
        name: SharedString,
        /// The chosen option.
        value: SharedString,
    },
    /// Free text, such as a prompt, under its name.
    Text {
        /// What it sets.
        name: SharedString,
        /// The text.
        text: SharedString,
    },
}

impl NodeField {
    /// A number row.
    pub fn number(
        name: impl Into<SharedString>,
        value: impl Into<SharedString>,
        fraction: f32,
    ) -> Self {
        Self::Number {
            name: name.into(),
            value: value.into(),
            fraction: fraction.clamp(0., 1.),
        }
    }

    /// A choice row.
    pub fn choice(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self::Choice {
            name: name.into(),
            value: value.into(),
        }
    }

    /// A text row.
    pub fn text(name: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self::Text {
            name: name.into(),
            text: text.into(),
        }
    }
}

type NodeBody = Rc<dyn Fn(NodeId, &mut Window, &mut App) -> AnyElement + 'static>;

/// One node: a titled card with its inputs on the leading edge, its outputs on the trailing
/// edge, then its fields and any body of the application's own.
#[derive(Clone)]
pub struct GraphNode {
    /// The header's words.
    pub title: SharedString,
    /// An icon before the title, usually the node's category.
    pub icon: Option<IconName>,
    /// The top-left corner on the canvas, before panning.
    pub position: Point<Pixels>,
    /// The card's width.
    pub width: Pixels,
    /// The ports on the leading edge.
    pub inputs: Vec<Port>,
    /// The ports on the trailing edge.
    pub outputs: Vec<Port>,
    /// Parameter rows under the ports.
    pub fields: Vec<NodeField>,
    /// Where it is in a run.
    pub state: NodeState,
    /// Added by the assistant and not yet kept: drawn in the agent colour, dashed, with Keep and
    /// Undo under it.
    pub by_agent: bool,
    /// Notes the person wrote on it, drawn at the bottom of the card. Line breaks are kept; up
    /// to [`COMMENT_LINES`] lines show, the rest is in the note's tooltip.
    pub comments: Vec<SharedString>,
    body: Option<NodeBody>,
}

/// How many lines of a comment a node shows; its tooltip has the whole text.
pub const COMMENT_LINES: usize = 4;

/// What a comment on a node shows: its first [`COMMENT_LINES`] lines as written, and an
/// ellipsis when there is more. Each line still wraps to the card's width.
pub fn comment_preview(text: &str) -> String {
    let mut lines = text.lines();
    let mut shown = lines
        .by_ref()
        .take(COMMENT_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if lines.next().is_some() {
        shown.push('…');
    }
    shown
}

impl GraphNode {
    /// A node titled `title` at the canvas origin, 200 px wide.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            position: point(px(0.), px(0.)),
            width: px(200.),
            inputs: Vec::new(),
            outputs: Vec::new(),
            fields: Vec::new(),
            state: NodeState::Idle,
            by_agent: false,
            comments: Vec::new(),
            body: None,
        }
    }

    /// Place it at `x`, `y` on the canvas.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.position = point(px(x), px(y));
        self
    }

    /// The card's width.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// An icon before the title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Add an input.
    pub fn input(mut self, name: impl Into<SharedString>, kind: usize) -> Self {
        self.inputs.push(Port::new(name, kind));
        self
    }

    /// Add an output.
    pub fn output(mut self, name: impl Into<SharedString>, kind: usize) -> Self {
        self.outputs.push(Port::new(name, kind));
        self
    }

    /// Add a parameter row.
    pub fn field(mut self, field: NodeField) -> Self {
        self.fields.push(field);
        self
    }

    /// Its state in a run.
    pub fn state(mut self, state: NodeState) -> Self {
        self.state = state;
        self
    }

    /// Mark it as the assistant's, until the person keeps or undoes it.
    pub fn by_agent(mut self, by_agent: bool) -> Self {
        self.by_agent = by_agent;
        self
    }

    /// A note on the node: "Under the vocal", "Chorus only". Several stack in order.
    pub fn comment(mut self, text: impl Into<SharedString>) -> Self {
        self.comments.push(text.into());
        self
    }

    /// Anything else at the bottom of the card, drawn each frame: a preview, a player.
    pub fn body(
        mut self,
        body: impl Fn(NodeId, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.body = Some(Rc::new(body));
        self
    }
}

/// What the person did on a [`NodeCanvas`]. The canvas has already applied it.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeCanvasEvent {
    /// A node was selected, or the selection cleared.
    Selected(Option<NodeId>),
    /// A node was dragged to a new place.
    Moved(NodeId),
    /// A wire was drawn. A wire it replaced on the same input comes first as `Disconnected`.
    Connected(Wire),
    /// A wire was picked up from its input and let go.
    Disconnected(Wire),
    /// The selected node was deleted with Delete or Backspace, and its wires with it.
    Removed(NodeId),
    /// The person kept a node the assistant added.
    Kept(NodeId),
    /// The person undid a node the assistant added: it is gone, and what it sat between is
    /// wired back together.
    Undone(NodeId),
    /// The person pressed one of a node's comments: open it for editing, over
    /// [`NodeCanvas::comment_key`] in the canvas's [`crate::SpotBook`].
    CommentPressed {
        /// The node it is on, which is now selected.
        node: NodeId,
        /// Which of its comments, from 0.
        index: usize,
    },
}

/// What the pointer is doing on the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    None,
    Pan(Point<Pixels>),
    Node(NodeId, Point<Pixels>, bool),
    /// A wire from an output, following the pointer (window coordinates).
    Wire {
        from: NodeId,
        output: usize,
        cursor: Point<Pixels>,
    },
}

/// Where the ports sit inside a node, so wires are drawn without measuring the nodes.
#[derive(Clone, Copy)]
struct Geometry {
    header: Pixels,
    row: Pixels,
    pad: Pixels,
}

impl Geometry {
    fn new(theme: &Theme) -> Self {
        Self {
            header: theme.density.row + px(4.),
            row: theme.density.control_sm + px(2.),
            pad: theme.spacing.xs,
        }
    }

    /// The centre of a port's socket, relative to the canvas before panning. The extra pixel
    /// is the node's border.
    fn port(&self, node: &GraphNode, index: usize, output: bool) -> Point<Pixels> {
        let x = if output {
            node.position.x + node.width
        } else {
            node.position.x
        };
        let y = node.position.y + px(1.) + self.header + self.pad + self.row * (index as f32 + 0.5);
        point(x, y)
    }
}

const DEFAULT_PAN: Point<Pixels> = point(px(16.), px(24.));

/// A node graph on a pannable canvas.
///
/// Hold it as an entity (`cx.new(|_| NodeCanvas::new("graph").port_types([..]))`), fill it
/// with [`NodeCanvas::add`] and [`NodeCanvas::connect`], and subscribe to its
/// [`NodeCanvasEvent`]s to keep the application's model in step. The person drags a node by
/// its card, draws a wire from an output to an input of the same type, picks a wire up again
/// by its input, pans with the background or the wheel, and deletes the selected node with
/// Delete. The touched node comes to the front.
pub struct NodeCanvas {
    id: ElementId,
    nodes: Vec<(NodeId, GraphNode)>,
    wires: Vec<Wire>,
    next: u64,
    pan: Point<Pixels>,
    selected: Option<NodeId>,
    drag: Drag,
    port_types: Vec<SharedString>,
    legend: bool,
    focus: Option<FocusHandle>,
    spots: Option<crate::SpotBook>,
}

impl EventEmitter<NodeCanvasEvent> for NodeCanvas {}

impl NodeCanvas {
    /// An empty canvas.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            nodes: Vec::new(),
            wires: Vec::new(),
            next: 0,
            pan: DEFAULT_PAN,
            selected: None,
            drag: Drag::None,
            port_types: Vec::new(),
            legend: true,
            focus: None,
            spots: None,
        }
    }

    /// Record where each node's comments are drawn in `book`, under
    /// [`NodeCanvas::comment_key`], so an [`crate::InlineEdit`] opens over the one pressed.
    pub fn spots(mut self, book: &crate::SpotBook) -> Self {
        self.spots = Some(book.clone());
        self
    }

    /// The key a comment's place is recorded under by [`NodeCanvas::spots`].
    pub fn comment_key(node: NodeId, index: usize) -> SharedString {
        format!("node-comment:{}:{index}", node.0).into()
    }

    /// The names of the port types, in the order of their indexes: the legend lists them and a
    /// socket's tooltip names its type. Up to five have their own colour.
    pub fn port_types<S: Into<SharedString>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.port_types = names.into_iter().map(Into::into).collect();
        self
    }

    /// Leave out the floating legend with "Reset view" and the port types.
    pub fn without_legend(mut self) -> Self {
        self.legend = false;
        self
    }

    // Reading.

    /// The nodes, back to front.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &GraphNode)> {
        self.nodes.iter().map(|(id, node)| (*id, node))
    }

    /// One node.
    pub fn node(&self, id: NodeId) -> Option<&GraphNode> {
        self.nodes
            .iter()
            .find(|(node, _)| *node == id)
            .map(|(_, node)| node)
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut GraphNode> {
        self.nodes
            .iter_mut()
            .find(|(node, _)| *node == id)
            .map(|(_, node)| node)
    }

    /// Every wire.
    pub fn wires(&self) -> &[Wire] {
        &self.wires
    }

    /// How far the canvas is panned: add it to a canvas position to get a place in the view.
    pub fn pan(&self) -> Point<Pixels> {
        self.pan
    }

    /// The selected node.
    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// Where the wire into `id`'s input `input` comes from.
    pub fn source(&self, id: NodeId, input: usize) -> Option<(NodeId, usize)> {
        self.wires
            .iter()
            .find(|wire| wire.to == id && wire.input == input)
            .map(|wire| (wire.from, wire.output))
    }

    /// How many inputs, across the graph, have no wire.
    pub fn unconnected_inputs(&self) -> usize {
        self.nodes
            .iter()
            .map(|(id, node)| {
                (0..node.inputs.len())
                    .filter(|input| self.source(*id, *input).is_none())
                    .count()
            })
            .sum()
    }

    /// `id` and every node downstream of it: what has to run again when it changes.
    pub fn downstream(&self, id: NodeId) -> Vec<NodeId> {
        let mut found = vec![id];
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            for wire in self.wires.iter().filter(|wire| wire.from == id) {
                if !found.contains(&wire.to) {
                    found.push(wire.to);
                    pending.push(wire.to);
                }
            }
        }
        found
    }

    /// The nodes in an order where each comes after every node that feeds it. A cycle cannot
    /// be ordered; its nodes come last, in the order they were added.
    pub fn execution_order(&self) -> Vec<NodeId> {
        let mut remaining: Vec<NodeId> = self.nodes.iter().map(|(id, _)| *id).collect();
        remaining.sort();
        let mut order = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let ready = remaining.iter().position(|id| {
                self.wires
                    .iter()
                    .filter(|wire| wire.to == *id)
                    .all(|wire| !remaining.contains(&wire.from))
            });
            order.push(remaining.remove(ready.unwrap_or(0)));
        }
        order
    }

    /// The type a port carries.
    fn port_kind(&self, id: NodeId, index: usize, output: bool) -> Option<usize> {
        let node = self.node(id)?;
        let ports = if output { &node.outputs } else { &node.inputs };
        ports.get(index).map(|port| port.kind)
    }

    /// Whether a wire may join these ports: different nodes, ports that exist, the same type.
    pub fn can_connect(&self, wire: Wire) -> bool {
        wire.from != wire.to
            && self
                .port_kind(wire.from, wire.output, true)
                .is_some_and(|kind| self.port_kind(wire.to, wire.input, false) == Some(kind))
    }

    // Changing.

    /// Add a node and return its id.
    pub fn add(&mut self, node: GraphNode, cx: &mut Context<Self>) -> NodeId {
        let id = NodeId(self.next);
        self.next += 1;
        self.nodes.push((id, node));
        cx.notify();
        id
    }

    /// Change a node in place: its fields, its ports, its title.
    pub fn update_node(
        &mut self,
        id: NodeId,
        update: impl FnOnce(&mut GraphNode),
        cx: &mut Context<Self>,
    ) {
        if let Some(node) = self.node_mut(id) {
            update(node);
            cx.notify();
        }
    }

    /// Set a node's state in a run.
    pub fn set_state(&mut self, id: NodeId, state: NodeState, cx: &mut Context<Self>) {
        self.update_node(id, |node| node.state = state, cx);
    }

    /// Take a node out, and its wires with it.
    pub fn remove(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.nodes.retain(|(node, _)| *node != id);
        self.wires.retain(|wire| wire.from != id && wire.to != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        cx.notify();
    }

    /// Draw a wire, replacing any wire already on that input. Returns false, and changes
    /// nothing, when [`NodeCanvas::can_connect`] refuses it.
    pub fn connect(&mut self, wire: Wire, cx: &mut Context<Self>) -> bool {
        self.connect_reporting(wire, cx).is_some()
    }

    /// [`NodeCanvas::connect`], returning the wire it replaced.
    fn connect_reporting(&mut self, wire: Wire, cx: &mut Context<Self>) -> Option<Option<Wire>> {
        if !self.can_connect(wire) {
            return None;
        }
        let replaced = self
            .wires
            .iter()
            .position(|old| old.to == wire.to && old.input == wire.input)
            .map(|index| self.wires.remove(index));
        self.wires.push(wire);
        cx.notify();
        Some(replaced)
    }

    /// Take a wire away.
    pub fn disconnect(&mut self, wire: Wire, cx: &mut Context<Self>) {
        self.wires.retain(|old| *old != wire);
        cx.notify();
    }

    /// Select a node and bring it to the front, or clear the selection.
    pub fn select(&mut self, id: Option<NodeId>, cx: &mut Context<Self>) {
        self.selected = id;
        if let Some(index) = id.and_then(|id| self.nodes.iter().position(|(node, _)| *node == id)) {
            let node = self.nodes.remove(index);
            self.nodes.push(node);
        }
        cx.notify();
    }

    /// Keep a node the assistant added.
    pub fn keep(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.update_node(id, |node| node.by_agent = false, cx);
    }

    /// Undo a node the assistant added: take it out and wire what fed its first input to
    /// every input it fed.
    pub fn undo_agent_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let source = self
            .wires
            .iter()
            .find(|wire| wire.to == id)
            .map(|wire| (wire.from, wire.output));
        let targets: Vec<_> = self
            .wires
            .iter()
            .filter(|wire| wire.from == id)
            .map(|wire| (wire.to, wire.input))
            .collect();
        self.remove(id, cx);
        if let Some((from, output)) = source {
            for (to, input) in targets {
                self.connect(Wire::new(from, output, to, input), cx);
            }
        }
    }

    /// Put every node back in [`NodeState::Idle`].
    pub fn reset_states(&mut self, cx: &mut Context<Self>) {
        for (_, node) in &mut self.nodes {
            node.state = NodeState::Idle;
        }
        cx.notify();
    }

    /// Pan back to where the canvas started.
    pub fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.pan = DEFAULT_PAN;
        cx.notify();
    }

    // Pointer handling.

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = &self.focus {
            window.focus(focus);
        }
        if event.button == MouseButton::Left && self.selected.is_some() {
            self.selected = None;
            cx.emit(NodeCanvasEvent::Selected(None));
        }
        self.drag = Drag::Pan(event.position);
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button.is_none() {
            // The button came up outside the window.
            if self.drag != Drag::None {
                self.finish_drag(cx);
            }
            return;
        }
        match self.drag {
            Drag::None => return,
            Drag::Pan(last) => {
                self.pan += event.position - last;
                self.drag = Drag::Pan(event.position);
            }
            Drag::Node(id, last, _) => {
                if let Some(node) = self.node_mut(id) {
                    node.position += event.position - last;
                }
                self.drag = Drag::Node(id, event.position, true);
            }
            Drag::Wire { from, output, .. } => {
                self.drag = Drag::Wire {
                    from,
                    output,
                    cursor: event.position,
                };
            }
        }
        cx.notify();
    }

    /// The button came up away from an input: a node drag ends, a loose wire is dropped.
    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if let Drag::Node(id, _, true) = self.drag {
            cx.emit(NodeCanvasEvent::Moved(id));
        }
        self.drag = Drag::None;
        cx.notify();
    }

    fn input_mouse_down(
        &mut self,
        id: NodeId,
        input: usize,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        // Pulling a connected input picks its wire up again, to move or drop it.
        if let Some(index) = self
            .wires
            .iter()
            .position(|wire| wire.to == id && wire.input == input)
        {
            let wire = self.wires.remove(index);
            cx.emit(NodeCanvasEvent::Disconnected(wire));
            self.drag = Drag::Wire {
                from: wire.from,
                output: wire.output,
                cursor: event.position,
            };
        }
        cx.notify();
    }

    fn input_mouse_up(&mut self, id: NodeId, input: usize, cx: &mut Context<Self>) {
        if let Drag::Wire { from, output, .. } = self.drag {
            let wire = Wire::new(from, output, id, input);
            if let Some(replaced) = self.connect_reporting(wire, cx) {
                if let Some(replaced) = replaced {
                    cx.emit(NodeCanvasEvent::Disconnected(replaced));
                }
                cx.emit(NodeCanvasEvent::Connected(wire));
            }
        }
        self.drag = Drag::None;
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if matches!(key, "delete" | "backspace")
            && let Some(id) = self.selected
        {
            self.remove(id, cx);
            cx.emit(NodeCanvasEvent::Removed(id));
            cx.stop_propagation();
        } else if key == "escape" && self.selected.is_some() {
            self.selected = None;
            cx.emit(NodeCanvasEvent::Selected(None));
            cx.notify();
        }
    }

    // Drawing.

    fn child_id(&self, name: String) -> ElementId {
        ElementId::NamedChild(Box::new(self.id.clone()), name.into())
    }

    fn port_color(theme: &Theme, kind: usize) -> gpui::Hsla {
        let ports = &theme.colors.ports;
        ports[kind % ports.len()]
    }

    fn socket(
        &self,
        id: NodeId,
        node: &GraphNode,
        index: usize,
        output: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        const HIT: f32 = 18.;
        let port = if output {
            &node.outputs[index]
        } else {
            &node.inputs[index]
        };
        let color = Self::port_color(theme, port.kind);
        let connected = self.wires.iter().any(|wire| {
            if output {
                wire.from == id && wire.output == index
            } else {
                wire.to == id && wire.input == index
            }
        });
        // While a wire is being dragged, inputs it can't join step back.
        let compatible = match self.drag {
            Drag::Wire {
                from, output: o, ..
            } => output || self.can_connect(Wire::new(from, o, id, index)),
            _ => true,
        };
        let offset = -(px(1.) + px(HIT / 2.));
        let row = Geometry::new(theme).row;
        let tooltip: SharedString = match self.port_types.get(port.kind) {
            Some(kind) => format!("{} · {kind}", port.name).into(),
            None => port.name.clone(),
        };
        let element_id = self.child_id(format!(
            "{}-{}-{index}",
            if output { "out" } else { "in" },
            id.0
        ));
        selector(div().id(element_id.clone()), &element_id)
            .absolute()
            .top((row - px(HIT)) / 2.)
            .when(output, |hit| hit.right(offset))
            .when(!output, |hit| hit.left(offset))
            .size(px(HIT))
            .flex()
            .items_center()
            .justify_center()
            .cursor_crosshair()
            .opacity(if compatible { 1. } else { 0.3 })
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .child(
                div()
                    .size(px(10.))
                    .rounded_full()
                    .border_2()
                    .border_color(color)
                    .bg(if connected {
                        color
                    } else {
                        theme.colors.elevated
                    }),
            )
            .when(output, |hit| {
                hit.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.drag = Drag::Wire {
                            from: id,
                            output: index,
                            cursor: event.position,
                        };
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
            })
            .when(!output, |hit| {
                hit.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.input_mouse_down(id, index, event, cx);
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                        this.input_mouse_up(id, index, cx);
                        cx.stop_propagation();
                    }),
                )
            })
    }

    fn state_mark(id: NodeId, node: &GraphNode, theme: &Theme, cx: &App) -> AnyElement {
        let strings = cx.strings();
        let mark = div().flex().items_center().gap(theme.spacing.xs);
        let faint = |text: SharedString| {
            Label::new(text)
                .size(TextSize::XSmall)
                .color(TextColor::Faint)
        };
        match node.state {
            NodeState::Idle if node.by_agent => mark.child(
                Icon::new(IconName::Sparkles)
                    .size(px(12.))
                    .color(theme.colors.agent),
            ),
            NodeState::Idle => mark,
            NodeState::Queued => mark.child(faint(strings.task_states[0].clone())),
            NodeState::Running(progress) => mark
                .child(
                    Label::new(format!("{:.0} %", progress * 100.))
                        .size(TextSize::XSmall)
                        .mono()
                        .color(TextColor::Muted),
                )
                .child(Spinner::new(("node-spinner", id.0 as usize)).size(px(12.))),
            NodeState::Done(seconds) => mark
                .children(seconds.map(|seconds| {
                    Label::new(format!("{seconds:.1} s"))
                        .size(TextSize::XSmall)
                        .mono()
                        .color(TextColor::Faint)
                }))
                .child(
                    Icon::new(IconName::Check)
                        .size(px(12.))
                        .color(theme.colors.success),
                ),
            NodeState::Cached => mark.child(faint(strings.cached.clone())),
            NodeState::Failed => mark.child(
                Icon::new(IconName::Alert)
                    .size(px(12.))
                    .color(theme.colors.danger),
            ),
        }
        .into_any_element()
    }

    fn field(field: &NodeField, row: Pixels, theme: &Theme) -> AnyElement {
        let well = div()
            .relative()
            .rounded(theme.shape.radius_xs)
            .bg(theme.colors.sunken)
            .overflow_hidden();
        let name = |name: &SharedString| {
            Label::new(name.clone())
                .size(TextSize::XSmall)
                .color(TextColor::Muted)
        };
        match field {
            NodeField::Number {
                name: label,
                value,
                fraction,
            } => well
                .h(row)
                .flex()
                .items_center()
                .justify_between()
                .gap(theme.spacing.sm)
                .px(theme.spacing.sm)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(relative(*fraction))
                        .bg(theme.colors.hover),
                )
                .child(name(label))
                .child(Label::new(value.clone()).size(TextSize::XSmall).mono()),
            NodeField::Choice { name: label, value } => well
                .h(row)
                .flex()
                .items_center()
                .gap(theme.spacing.xs)
                .px(theme.spacing.sm)
                .child(name(label))
                .child(div().flex_1())
                .child(Label::new(value.clone()).size(TextSize::XSmall))
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size(px(10.))
                        .color(theme.colors.text_muted),
                ),
            NodeField::Text { name: label, text } => well
                .flex()
                .flex_col()
                .gap(theme.spacing.xxs)
                .p(theme.spacing.sm)
                .child(name(label))
                .child(Label::new(text.clone()).size(TextSize::XSmall)),
        }
        .into_any_element()
    }

    fn render_node(
        &self,
        id: NodeId,
        node: &GraphNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let strings = cx.strings().clone();
        let geometry = Geometry::new(&theme);
        let selected = self.selected == Some(id);
        let running = matches!(node.state, NodeState::Running(_));
        let dragging = matches!(self.drag, Drag::Node(dragged, _, _) if dragged == id);
        let rows = node.inputs.len().max(node.outputs.len());
        let border = if node.by_agent {
            theme.colors.agent
        } else if node.state == NodeState::Failed {
            theme.colors.danger
        } else if selected || running {
            theme.colors.accent
        } else {
            theme.colors.border_strong
        };
        let inner = Theme::concentric(theme.shape.radius_md, px(1.));

        let header = div()
            .h(geometry.header)
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .px(theme.spacing.sm)
            .rounded_t(inner)
            .border_b_1()
            .border_color(theme.colors.separator)
            .when(selected, |header| header.bg(theme.colors.selected))
            .children(
                node.icon
                    .map(|icon| Icon::new(icon).size(px(13.)).color(theme.colors.text_muted)),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Label::new(node.title.clone())
                        .size(TextSize::Small)
                        .weight(theme.typography.label_weight)
                        .truncate(),
                ),
            )
            .child(Self::state_mark(id, node, &theme, cx));

        let port_label = |port: Option<&Port>| match port {
            Some(port) => Label::new(port.name.clone())
                .size(TextSize::XSmall)
                .color(TextColor::Muted)
                .into_any_element(),
            None => div().into_any_element(),
        };
        let ports = div()
            .flex()
            .flex_col()
            .pt(geometry.pad)
            .when(rows == 0, |ports| ports.pt(px(0.)))
            .children((0..rows).map(|row| {
                div()
                    .relative()
                    .h(geometry.row)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(theme.spacing.md)
                    .child(port_label(node.inputs.get(row)))
                    .child(port_label(node.outputs.get(row)))
                    .when(row < node.inputs.len(), |r| {
                        r.child(self.socket(id, node, row, false, &theme, cx))
                    })
                    .when(row < node.outputs.len(), |r| {
                        r.child(self.socket(id, node, row, true, &theme, cx))
                    })
            }));

        let fields = (!node.fields.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xs)
                .px(theme.spacing.sm)
                .pt(theme.spacing.xs)
                .pb(theme.spacing.sm)
                .children(
                    node.fields
                        .iter()
                        .map(|field| Self::field(field, geometry.row, &theme)),
                )
        });
        let body = node.body.as_ref().map(|body| {
            div()
                .px(theme.spacing.sm)
                .pb(theme.spacing.sm)
                .child(body(id, window, cx))
        });

        let comments = (!node.comments.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xs)
                .px(theme.spacing.sm)
                .pb(theme.spacing.sm)
                .children(node.comments.iter().enumerate().map(|(index, text)| {
                    let note_id = self.child_id(format!("comment-{}-{index}", id.0));
                    let whole = text.clone();
                    selector(div().id(note_id.clone()), &note_id)
                        .relative()
                        .flex()
                        .gap(theme.spacing.xs)
                        .p(theme.spacing.xs)
                        .rounded(theme.shape.radius_xs)
                        .bg(theme.colors.warning_wash)
                        .cursor_pointer()
                        .hover(|note| note.bg(theme.colors.hover))
                        .child(
                            div()
                                .w(px(3.))
                                .flex_none()
                                .rounded(px(1.5))
                                .bg(theme.colors.warning),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(theme.typography.size_xs)
                                .text_color(theme.colors.text)
                                .child(comment_preview(text)),
                        )
                        .children(self.spots.as_ref().map(|book| {
                            book.spot(Self::comment_key(id, index))
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                        }))
                        .tooltip(move |window, cx| Tooltip::new(whole.clone()).build(window, cx))
                        // A note has nowhere to be dragged to: a press opens it, and never also
                        // takes hold of the node.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                if this.selected != Some(id) {
                                    this.select(Some(id), cx);
                                    cx.emit(NodeCanvasEvent::Selected(Some(id)));
                                }
                                cx.emit(NodeCanvasEvent::CommentPressed { node: id, index });
                            }),
                        )
                }))
        });

        let agent_footer = node.by_agent.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xs)
                .px(theme.spacing.sm)
                .py(theme.spacing.sm)
                .rounded_b(inner)
                .border_t_1()
                .border_color(theme.colors.separator)
                .bg(theme.colors.agent_wash)
                .child(
                    Label::new(strings.added_by_agent.clone())
                        .size(TextSize::XSmall)
                        .color(TextColor::Agent),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(theme.spacing.sm)
                        .child(
                            Button::new(
                                self.child_id(format!("undo-{}", id.0)),
                                strings.undo.clone(),
                            )
                            .ghost()
                            .icon(IconName::Undo)
                            .size(ControlSize::Small)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.undo_agent_node(id, cx);
                                    cx.emit(NodeCanvasEvent::Undone(id));
                                },
                            )),
                        )
                        .child(
                            Button::new(
                                self.child_id(format!("keep-{}", id.0)),
                                strings.keep.clone(),
                            )
                            .size(ControlSize::Small)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.keep(id, cx);
                                    cx.emit(NodeCanvasEvent::Kept(id));
                                },
                            )),
                        ),
                )
        });

        let element_id = self.child_id(format!("node-{}", id.0));
        selector(div().id(element_id.clone()), &element_id)
            .absolute()
            .left(self.pan.x + node.position.x)
            .top(self.pan.y + node.position.y)
            .w(node.width)
            .flex()
            .flex_col()
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(border)
            .when(node.by_agent, |card| card.border_dashed())
            .shadow(if dragging {
                theme.elevation.medium.clone()
            } else {
                theme.elevation.low.clone()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if let Some(focus) = &this.focus {
                        window.focus(focus);
                    }
                    let changed = this.selected != Some(id);
                    this.select(Some(id), cx);
                    if changed {
                        cx.emit(NodeCanvasEvent::Selected(Some(id)));
                    }
                    this.drag = Drag::Node(id, event.position, false);
                    cx.stop_propagation();
                }),
            )
            .child(header)
            .when(running, |card| {
                card.child(
                    div().h(px(2.)).bg(theme.colors.accent_wash).child(
                        div()
                            .h_full()
                            .w(relative(match node.state {
                                NodeState::Running(progress) => progress.clamp(0., 1.),
                                _ => 0.,
                            }))
                            .bg(theme.colors.accent),
                    ),
                )
            })
            .child(ports)
            .children(fields)
            .children(body)
            .children(comments)
            .children(agent_footer)
            .into_any_element()
    }

    /// The dot grid and the wires, painted under the nodes.
    fn paint_wires(&self, theme: &Theme) -> impl IntoElement + use<> {
        let geometry = Geometry::new(theme);
        let pan = self.pan;
        let highlight = self.selected;
        let wires: Vec<_> = self
            .wires
            .iter()
            .filter_map(|wire| {
                let from = self.node(wire.from)?;
                let to = self.node(wire.to)?;
                let kind = self.port_kind(wire.from, wire.output, true)?;
                let strong = highlight == Some(wire.from) || highlight == Some(wire.to);
                Some((
                    pan + geometry.port(from, wire.output, true),
                    pan + geometry.port(to, wire.input, false),
                    Self::port_color(theme, kind),
                    strong,
                ))
            })
            .collect();
        let pending = match self.drag {
            Drag::Wire {
                from,
                output,
                cursor,
            } => self.node(from).and_then(|node| {
                let kind = self.port_kind(from, output, true)?;
                Some((
                    pan + geometry.port(node, output, true),
                    cursor,
                    Self::port_color(theme, kind),
                ))
            }),
            _ => None,
        };
        let dot = theme.colors.grid;
        let spacing = px(24.);
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let origin = bounds.origin;
                let start_x = pan.x % spacing;
                let start_y = pan.y % spacing;
                let mut y = start_y;
                while y < bounds.size.height {
                    let mut x = start_x;
                    while x < bounds.size.width {
                        window.paint_quad(fill(
                            Bounds::new(origin + point(x, y), size(px(1.5), px(1.5))),
                            dot,
                        ));
                        x += spacing;
                    }
                    y += spacing;
                }
                for (from, to, color, strong) in &wires {
                    let width = if *strong { px(3.) } else { px(2.) };
                    if let Some(path) = bezier(origin + *from, origin + *to, width) {
                        window.paint_path(path, if *strong { *color } else { color.opacity(0.8) });
                    }
                }
                if let Some((from, cursor, color)) = pending
                    && let Some(path) = bezier(origin + from, cursor, px(2.))
                {
                    window.paint_path(path, color);
                }
            },
        )
        .size_full()
    }

    fn legend(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let strings = cx.strings();
        div()
            .absolute()
            .left(theme.spacing.lg)
            .bottom(theme.spacing.lg)
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .px(theme.spacing.md)
            .py(theme.spacing.xs)
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .shadow(theme.elevation.medium.clone())
            // The legend is not part of the canvas: a press on it neither pans nor deselects.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(
                    self.child_id("reset-view".into()),
                    strings.reset_view.clone(),
                )
                .ghost()
                .size(ControlSize::Small)
                .on_click(cx.listener(|this, _, _, cx| this.reset_view(cx))),
            )
            .when(!self.port_types.is_empty(), |legend| {
                legend.child(Divider::vertical())
            })
            .children(self.port_types.iter().enumerate().map(|(kind, name)| {
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(StatusDot::new(Self::port_color(theme, kind)))
                    .child(
                        Label::new(name.clone())
                            .size(TextSize::XSmall)
                            .color(TextColor::Muted),
                    )
            }))
    }
}

/// A wire from an output to an input, leaving and entering horizontally.
fn bezier(from: Point<Pixels>, to: Point<Pixels>, width: Pixels) -> Option<gpui::Path<Pixels>> {
    let reach = ((to.x - from.x).abs() * 0.5).max(px(48.));
    let mut builder = PathBuilder::stroke(width);
    builder.move_to(from);
    builder.cubic_bezier_to(to, point(from.x + reach, from.y), point(to.x - reach, to.y));
    builder.build().ok()
}

impl Render for NodeCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.get_or_insert_with(|| cx.focus_handle()).clone();
        let theme = cx.theme().clone();
        let order: Vec<NodeId> = self.nodes.iter().map(|(id, _)| *id).collect();
        let mut nodes = Vec::with_capacity(order.len());
        for id in order {
            // Cloning the node lets its body closure borrow the app while it draws.
            if let Some(node) = self.node(id).cloned() {
                nodes.push(self.render_node(id, &node, window, cx));
            }
        }
        let legend = self.legend.then(|| self.legend(cx));
        selector(div().id(self.id.clone()), &self.id)
            .track_focus(&focus)
            .relative()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .bg(theme.colors.background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, window, cx| this.mouse_down(event, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event, window, cx| this.mouse_down(event, window, cx)),
            )
            .on_mouse_move(cx.listener(|this, event, _, cx| this.mouse_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.finish_drag(cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.finish_drag(cx)),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                this.pan += event.delta.pixel_delta(px(20.));
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event, _, cx| this.key_down(event, cx)))
            .child(div().absolute().inset_0().child(self.paint_wires(&theme)))
            .children(nodes)
            .children(legend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> (NodeCanvas, [NodeId; 3]) {
        let mut canvas = NodeCanvas::new("graph");
        let mut ids = [NodeId(0); 3];
        for (index, node) in [
            GraphNode::new("Source").output("Audio", 0),
            GraphNode::new("Effect").input("In", 0).output("Out", 0),
            GraphNode::new("Sink").input("In", 0).input("Gain", 1),
        ]
        .into_iter()
        .enumerate()
        {
            ids[index] = NodeId(canvas.next);
            canvas.next += 1;
            canvas.nodes.push((ids[index], node));
        }
        (canvas, ids)
    }

    #[test]
    fn wires_join_ports_of_one_type_on_two_nodes() {
        let (canvas, [source, effect, sink]) = graph();
        assert!(canvas.can_connect(Wire::new(source, 0, effect, 0)));
        assert!(!canvas.can_connect(Wire::new(source, 0, sink, 1)));
        assert!(!canvas.can_connect(Wire::new(effect, 0, effect, 0)));
        assert!(!canvas.can_connect(Wire::new(source, 3, sink, 0)));
    }

    #[test]
    fn order_puts_sources_first_and_downstream_follows_wires() {
        let (mut canvas, [source, effect, sink]) = graph();
        canvas.wires.push(Wire::new(effect, 0, sink, 0));
        canvas.wires.push(Wire::new(source, 0, effect, 0));
        assert_eq!(canvas.execution_order(), [source, effect, sink]);
        assert_eq!(canvas.downstream(effect), [effect, sink]);
        assert_eq!(canvas.unconnected_inputs(), 1);
        assert_eq!(canvas.source(sink, 0), Some((effect, 0)));
    }
}

#[cfg(test)]
mod comment_tests {
    use super::comment_preview;

    #[test]
    fn a_comment_keeps_four_lines_and_says_when_there_is_more() {
        assert_eq!(comment_preview("Under the vocal"), "Under the vocal");
        assert_eq!(comment_preview("a\nb\nc\nd"), "a\nb\nc\nd");
        assert_eq!(comment_preview("a\nb\nc\nd\ne"), "a\nb\nc\nd…");
    }
}
