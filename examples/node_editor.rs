//! A mock node-based workflow editor in the shape of ComfyUI: a library of nodes on the left, a
//! graph on a pannable canvas, and the selected node's parameters and the run queue on the right.
//! Nodes can be dragged, wired output to input, added from the library and run in order; the run
//! is simulated and nothing is generated.
//!
//! ```text
//! cargo run --example node_editor -- [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use std::time::Duration;

use atelier::agent::{
    AgentPanel, AgentStatus, AgentToggle, AskBar, AssistantMessage, Composer, ContextChip,
    ToolCall, ToolGroup, ToolState, UserMessage,
};
use atelier::{
    ActiveTheme, Appearance, AudioPlayer, Badge, Button, ButtonVariant, ControlSize, Direction,
    Disclosure, Divider, Edge, Icon, IconName, Label, ListItem, ProviderLogo, SectionHeader,
    Segment, SegmentedControl, Sidebar, Skeleton, Slider, Spinner, StatusDot, TaskProgress,
    TextColor, TextField, TextSize, Theme, TitleBar, Tone, Tooltip, WindowFrame,
};
use common::Themed;
use gpui::{
    Bounds, Context, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, PathBuilder, Pixels, Point, Render, ScrollWheelEvent, SharedString, Styled,
    Task, Window, canvas, div, fill, point, prelude::*, px, relative, size,
};

/// How long one simulated unit of work takes.
const TICK: Duration = Duration::from_millis(70);

/// The type of value a port carries. Wires only join ports of the same type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataType {
    Model,
    Conditioning,
    Latent,
    Audio,
    Value,
}

impl DataType {
    const ALL: [Self; 5] = [
        Self::Model,
        Self::Conditioning,
        Self::Latent,
        Self::Audio,
        Self::Value,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Conditioning => "Conditioning",
            Self::Latent => "Latent",
            Self::Audio => "Audio",
            Self::Value => "Value",
        }
    }

    fn color(self, theme: &Theme) -> Hsla {
        theme.colors.ports[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Loaders,
    Prompts,
    Sampling,
    Audio,
    Output,
}

impl Category {
    const ALL: [Self; 5] = [
        Self::Loaders,
        Self::Prompts,
        Self::Sampling,
        Self::Audio,
        Self::Output,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Loaders => "Loaders",
            Self::Prompts => "Prompts",
            Self::Sampling => "Sampling",
            Self::Audio => "Audio",
            Self::Output => "Output",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Loaders => IconName::Folder,
            Self::Prompts => IconName::File,
            Self::Sampling => IconName::Brain,
            Self::Audio => IconName::Waveform,
            Self::Output => IconName::Music,
        }
    }
}

/// A node parameter, drawn as a widget row in the node and as a control in the inspector.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Param {
    /// A number shown as a filled bar: its position in the range and its formatted value.
    Number(f32, &'static str),
    /// One of a few options.
    Choice(&'static [&'static str], usize),
    /// Free text, such as a prompt.
    Text(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    LoadModel,
    Prompt,
    NegativePrompt,
    TempoKey,
    EmptyLatent,
    Sampler,
    Decode,
    Normalize,
    Reverb,
    StemSplit,
    Preview,
    Save,
}

/// What a kind of node takes, gives and lets you set.
struct Spec {
    title: &'static str,
    category: Category,
    inputs: &'static [(&'static str, DataType)],
    outputs: &'static [(&'static str, DataType)],
    params: &'static [(&'static str, Param)],
    /// Simulated work, in ticks; the sampler reports its steps.
    work: usize,
}

impl Kind {
    const ALL: [Self; 12] = [
        Self::LoadModel,
        Self::Prompt,
        Self::NegativePrompt,
        Self::TempoKey,
        Self::EmptyLatent,
        Self::Sampler,
        Self::Decode,
        Self::Normalize,
        Self::Reverb,
        Self::StemSplit,
        Self::Preview,
        Self::Save,
    ];

    fn spec(self) -> Spec {
        use DataType::*;
        match self {
            Self::LoadModel => Spec {
                title: "Load model",
                category: Category::Loaders,
                inputs: &[],
                outputs: &[("Model", Model), ("Text encoder", Model), ("Codec", Model)],
                params: &[(
                    "Checkpoint",
                    Param::Choice(&["auris-music-v2", "auris-music-v1"], 0),
                )],
                work: 12,
            },
            Self::Prompt => Spec {
                title: "Prompt",
                category: Category::Prompts,
                inputs: &[("Text encoder", Model), ("Tempo", Value), ("Key", Value)],
                outputs: &[("Conditioning", Conditioning)],
                params: &[(
                    "Text",
                    Param::Text("Warm analog pad, slow attack, tape hiss, wide stereo"),
                )],
                work: 4,
            },
            Self::NegativePrompt => Spec {
                title: "Negative prompt",
                category: Category::Prompts,
                inputs: &[("Text encoder", Model)],
                outputs: &[("Conditioning", Conditioning)],
                params: &[("Text", Param::Text("Drums, vocals, harsh highs"))],
                work: 3,
            },
            Self::TempoKey => Spec {
                title: "Tempo and key",
                category: Category::Prompts,
                inputs: &[],
                outputs: &[("Tempo", Value), ("Key", Value)],
                params: &[
                    ("Tempo", Param::Number(0.42, "84 BPM")),
                    ("Key", Param::Choice(&["D minor", "F major", "A minor"], 0)),
                ],
                work: 1,
            },
            Self::EmptyLatent => Spec {
                title: "Empty latent",
                category: Category::Sampling,
                inputs: &[("Tempo", Value)],
                outputs: &[("Latent", Latent)],
                params: &[
                    ("Length", Param::Number(0.25, "0:32")),
                    ("Batch", Param::Number(0.25, "2")),
                ],
                work: 1,
            },
            Self::Sampler => Spec {
                title: "Sampler",
                category: Category::Sampling,
                inputs: &[
                    ("Model", Model),
                    ("Positive", Conditioning),
                    ("Negative", Conditioning),
                    ("Latent", Latent),
                ],
                outputs: &[("Latent", Latent)],
                params: &[
                    ("Seed", Param::Number(0.48, "482 113")),
                    ("Steps", Param::Number(0.3, "30")),
                    ("Guidance", Param::Number(0.45, "4.5")),
                    ("Sampler", Param::Choice(&["Euler", "DPM++ 2M", "DDIM"], 1)),
                ],
                work: 30,
            },
            Self::Decode => Spec {
                title: "Decode audio",
                category: Category::Audio,
                inputs: &[("Latent", Latent), ("Codec", Model)],
                outputs: &[("Audio", Audio)],
                params: &[],
                work: 6,
            },
            Self::Normalize => Spec {
                title: "Loudness normalize",
                category: Category::Audio,
                inputs: &[("Audio", Audio)],
                outputs: &[("Audio", Audio)],
                params: &[
                    ("Target", Param::Number(0.6, "−14 LUFS")),
                    ("True peak", Param::Number(0.8, "−1.0 dBTP")),
                ],
                work: 3,
            },
            Self::Reverb => Spec {
                title: "Reverb",
                category: Category::Audio,
                inputs: &[("Audio", Audio)],
                outputs: &[("Audio", Audio)],
                params: &[
                    ("Size", Param::Number(0.7, "70 %")),
                    ("Mix", Param::Number(0.25, "25 %")),
                ],
                work: 4,
            },
            Self::StemSplit => Spec {
                title: "Split stems",
                category: Category::Audio,
                inputs: &[("Audio", Audio)],
                outputs: &[("Bass", Audio), ("Harmony", Audio), ("Texture", Audio)],
                params: &[("Quality", Param::Choice(&["Fast", "Best"], 1))],
                work: 10,
            },
            Self::Preview => Spec {
                title: "Preview audio",
                category: Category::Output,
                inputs: &[("Audio", Audio)],
                outputs: &[],
                params: &[],
                work: 1,
            },
            Self::Save => Spec {
                title: "Save audio",
                category: Category::Output,
                inputs: &[("Audio", Audio)],
                outputs: &[],
                params: &[
                    ("File name", Param::Text("pad_take_%n.wav")),
                    ("Format", Param::Choice(&["WAV", "FLAC", "MP3"], 0)),
                ],
                work: 2,
            },
        }
    }

    fn width(self) -> Pixels {
        match self {
            Self::Preview => px(240.),
            Self::Prompt | Self::NegativePrompt | Self::Sampler => px(220.),
            _ => px(200.),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NodeState {
    Idle,
    Queued,
    /// Running, with progress from 0 to 1.
    Running(f32),
    /// Finished in this many seconds.
    Done(f32),
    /// Nothing upstream changed, so the last result was reused.
    Cached,
}

struct Node {
    id: usize,
    kind: Kind,
    /// Top-left corner on the canvas, before panning.
    position: Point<Pixels>,
    params: Vec<(&'static str, Param)>,
    /// Added by the assistant and not yet kept.
    by_agent: bool,
    state: NodeState,
    /// Has produced a result that a later run could reuse.
    has_result: bool,
    /// A parameter changed since the last run.
    dirty: bool,
}

impl Node {
    fn new(id: usize, kind: Kind, x: f32, y: f32) -> Self {
        Self {
            id,
            kind,
            position: point(px(x), px(y)),
            params: kind.spec().params.to_vec(),
            by_agent: false,
            state: NodeState::Idle,
            has_result: false,
            dirty: false,
        }
    }
}

/// A wire from one node's output to another node's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Link {
    from: usize,
    output: usize,
    to: usize,
    input: usize,
}

/// What the pointer is doing on the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    None,
    Pan(Point<Pixels>),
    Node(usize, Point<Pixels>),
    /// A wire from an output, following the pointer (window coordinates).
    Wire {
        from: usize,
        output: usize,
        cursor: Point<Pixels>,
    },
}

/// Where the ports sit inside a node, so wires can be drawn without measuring the nodes.
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
    fn port(&self, node: &Node, index: usize, output: bool) -> Point<Pixels> {
        let x = if output {
            node.position.x + node.kind.width()
        } else {
            node.position.x
        };
        let y = node.position.y + px(1.) + self.header + self.pad + self.row * (index as f32 + 0.5);
        point(x, y)
    }
}

/// A finished run, for the queue history.
struct RunRecord {
    number: usize,
    seconds: f32,
    reused: usize,
}

struct NodeEditor {
    choice: (Direction, Appearance),
    nodes: Vec<Node>,
    links: Vec<Link>,
    next_id: usize,
    pan: Point<Pixels>,
    selected: Option<usize>,
    drag: Drag,
    collapsed: Vec<Category>,
    agent_open: bool,
    run: Option<Task<()>>,
    run_progress: (usize, usize),
    running_title: &'static str,
    history: Vec<RunRecord>,
}

impl Themed for NodeEditor {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl NodeEditor {
    fn new(choice: (Direction, Appearance)) -> Self {
        let mut nodes = vec![
            Node::new(0, Kind::LoadModel, 0., 20.),
            Node::new(1, Kind::TempoKey, 0., 250.),
            Node::new(2, Kind::Prompt, 228., 0.),
            Node::new(3, Kind::NegativePrompt, 228., 236.),
            Node::new(4, Kind::EmptyLatent, 228., 410.),
            Node::new(5, Kind::Sampler, 476., 60.),
            Node::new(6, Kind::Decode, 724., 60.),
            Node::new(7, Kind::Normalize, 724., 200.),
            Node::new(8, Kind::Preview, 952., 30.),
            Node::new(9, Kind::Save, 952., 250.),
        ];
        nodes[7].by_agent = true;
        let link = |from, output, to, input| Link {
            from,
            output,
            to,
            input,
        };
        Self {
            choice,
            nodes,
            links: vec![
                link(0, 1, 2, 0),
                link(1, 0, 2, 1),
                link(1, 1, 2, 2),
                link(0, 1, 3, 0),
                link(1, 0, 4, 0),
                link(0, 0, 5, 0),
                link(2, 0, 5, 1),
                link(3, 0, 5, 2),
                link(4, 0, 5, 3),
                link(5, 0, 6, 0),
                link(0, 2, 6, 1),
                link(6, 0, 7, 0),
                link(7, 0, 8, 0),
                link(7, 0, 9, 0),
            ],
            next_id: 10,
            pan: point(px(16.), px(24.)),
            selected: Some(5),
            drag: Drag::None,
            collapsed: Vec::new(),
            agent_open: false,
            run: None,
            run_progress: (0, 0),
            running_title: "",
            history: vec![RunRecord {
                number: 1,
                seconds: 6.8,
                reused: 0,
            }],
        }
    }

    fn node(&self, id: usize) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    fn node_mut(&mut self, id: usize) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    fn data_type(&self, id: usize, index: usize, output: bool) -> Option<DataType> {
        let spec = self.node(id)?.kind.spec();
        let ports = if output { spec.outputs } else { spec.inputs };
        ports.get(index).map(|(_, data)| *data)
    }

    // Graph edits.

    fn select(&mut self, id: usize) {
        self.selected = Some(id);
        // The touched node comes to the front, as it does in ComfyUI.
        if let Some(index) = self.nodes.iter().position(|node| node.id == id) {
            let node = self.nodes.remove(index);
            self.nodes.push(node);
        }
    }

    fn connect(&mut self, from: usize, output: usize, to: usize, input: usize) {
        if from == to || self.data_type(from, output, true) != self.data_type(to, input, false) {
            return;
        }
        // An input takes one wire; the new one replaces it.
        self.links
            .retain(|link| !(link.to == to && link.input == input));
        self.links.push(Link {
            from,
            output,
            to,
            input,
        });
        self.mark_dirty(to);
    }

    fn add_node(&mut self, kind: Kind) {
        let id = self.next_id;
        self.next_id += 1;
        // Near the top-left of the visible canvas, stepped so new nodes don't stack exactly.
        let step = (id % 6) as f32 * 24.;
        let position = point(px(160. + step), px(80. + step)) - self.pan;
        let mut node = Node::new(id, kind, 0., 0.);
        node.position = position;
        self.nodes.push(node);
        self.selected = Some(id);
    }

    fn remove_node(&mut self, id: usize) {
        self.nodes.retain(|node| node.id != id);
        self.links.retain(|link| link.from != id && link.to != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
    }

    /// Undo the assistant's node: take it out and wire what it sat between back together.
    fn undo_agent_node(&mut self, id: usize) {
        let source = self
            .links
            .iter()
            .find(|link| link.to == id)
            .map(|link| (link.from, link.output));
        let targets: Vec<_> = self
            .links
            .iter()
            .filter(|link| link.from == id)
            .map(|link| (link.to, link.input))
            .collect();
        self.remove_node(id);
        if let Some((from, output)) = source {
            for (to, input) in targets {
                self.connect(from, output, to, input);
            }
        }
    }

    /// A changed node, and everything downstream of it, has to run again.
    fn mark_dirty(&mut self, id: usize) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if let Some(node) = self.node_mut(id) {
                if node.dirty {
                    continue;
                }
                node.dirty = true;
            }
            pending.extend(
                self.links
                    .iter()
                    .filter(|link| link.from == id)
                    .map(|link| link.to),
            );
        }
    }

    // Running.

    /// Nodes in an order where every node comes after the nodes that feed it.
    fn execution_order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        let mut remaining: Vec<usize> = self.nodes.iter().map(|node| node.id).collect();
        remaining.sort();
        while !remaining.is_empty() {
            let ready = remaining.iter().position(|id| {
                self.links
                    .iter()
                    .filter(|link| link.to == *id)
                    .all(|link| !remaining.contains(&link.from))
            });
            // A cycle leaves nothing ready; run the rest in id order rather than hang.
            let index = ready.unwrap_or(0);
            order.push(remaining.remove(index));
        }
        order
    }

    fn start_run(&mut self, cx: &mut Context<Self>) {
        let order = self.execution_order();
        for node in &mut self.nodes {
            node.state = NodeState::Queued;
        }
        self.run_progress = (0, order.len());
        let number = self.history.len() + 1;
        self.run = Some(cx.spawn(async move |this, cx| {
            let mut seconds = 0.;
            let mut reused = 0;
            for (index, id) in order.iter().copied().enumerate() {
                // Decide whether the node runs or reuses its result, as ComfyUI's cache does.
                let Ok(work) = this.update(cx, |this, cx| {
                    this.run_progress.0 = index;
                    let Some(node) = this.node_mut(id) else {
                        return 0;
                    };
                    let spec = node.kind.spec();
                    let reuse = node.has_result && !node.dirty && node.kind != Kind::Sampler;
                    node.state = if reuse {
                        NodeState::Cached
                    } else {
                        NodeState::Running(0.)
                    };
                    this.running_title = spec.title;
                    cx.notify();
                    if reuse { 0 } else { spec.work }
                }) else {
                    return;
                };
                if work == 0 {
                    reused += 1;
                    continue;
                }
                for tick in 1..=work {
                    cx.background_executor().timer(TICK).await;
                    let progress = tick as f32 / work as f32;
                    if this
                        .update(cx, |this, cx| {
                            if let Some(node) = this.node_mut(id) {
                                node.state = NodeState::Running(progress);
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                let took = work as f32 * TICK.as_secs_f32();
                seconds += took;
                let _ = this.update(cx, |this, cx| {
                    if let Some(node) = this.node_mut(id) {
                        node.state = NodeState::Done(took);
                        node.has_result = true;
                        node.dirty = false;
                    }
                    cx.notify();
                });
            }
            let _ = this.update(cx, |this, cx| {
                this.run = None;
                this.history.push(RunRecord {
                    number,
                    seconds,
                    reused,
                });
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn stop_run(&mut self, cx: &mut Context<Self>) {
        // Dropping the task cancels it.
        self.run = None;
        for node in &mut self.nodes {
            if matches!(node.state, NodeState::Queued | NodeState::Running(_)) {
                node.state = NodeState::Idle;
            }
        }
        cx.notify();
    }

    // Pointer handling on the canvas.

    fn canvas_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left {
            self.selected = None;
        }
        self.drag = Drag::Pan(event.position);
        cx.notify();
    }

    fn canvas_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button.is_none() {
            // The button came up outside the window.
            if self.drag != Drag::None {
                self.drag = Drag::None;
                cx.notify();
            }
            return;
        }
        match self.drag {
            Drag::None => return,
            Drag::Pan(last) => {
                self.pan += event.position - last;
                self.drag = Drag::Pan(event.position);
            }
            Drag::Node(id, last) => {
                if let Some(node) = self.node_mut(id) {
                    node.position += event.position - last;
                }
                self.drag = Drag::Node(id, event.position);
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

    fn canvas_mouse_up(&mut self, cx: &mut Context<Self>) {
        // A wire let go anywhere but on an input is dropped.
        self.drag = Drag::None;
        cx.notify();
    }

    fn canvas_scroll(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        self.pan += event.delta.pixel_delta(px(20.));
        cx.notify();
    }

    fn input_mouse_down(&mut self, id: usize, input: usize, event: &MouseDownEvent) {
        // Pulling a connected input picks its wire up again, to move or drop it.
        if let Some(index) = self
            .links
            .iter()
            .position(|link| link.to == id && link.input == input)
        {
            let link = self.links.remove(index);
            self.mark_dirty(id);
            self.drag = Drag::Wire {
                from: link.from,
                output: link.output,
                cursor: event.position,
            };
        }
    }

    fn input_mouse_up(&mut self, id: usize, input: usize) {
        if let Drag::Wire { from, output, .. } = self.drag {
            self.connect(from, output, id, input);
        }
        self.drag = Drag::None;
    }

    // Rendering.

    fn library(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let mut list = div()
            .id("library-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(theme.spacing.xxs);
        for category in Category::ALL {
            let open = !self.collapsed.contains(&category);
            list = list.child(
                Disclosure::new(("category", category as usize), category.label(), open).on_toggle(
                    cx.listener(move |this, _, _, cx| {
                        if let Some(index) = this.collapsed.iter().position(|c| *c == category) {
                            this.collapsed.remove(index);
                        } else {
                            this.collapsed.push(category);
                        }
                        cx.notify();
                    }),
                ),
            );
            if !open {
                continue;
            }
            for kind in Kind::ALL {
                let spec = kind.spec();
                if spec.category != category {
                    continue;
                }
                let count = self.nodes.iter().filter(|node| node.kind == kind).count();
                list = list.child(
                    ListItem::new(("library", kind as usize), spec.title)
                        .icon(category.icon())
                        .indent(1)
                        .when(count > 0, |item| {
                            item.trailing(
                                Label::new(count.to_string())
                                    .size(TextSize::XSmall)
                                    .mono()
                                    .color(TextColor::Faint),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_node(kind);
                            cx.notify();
                        })),
                );
            }
        }
        Sidebar::new(Edge::Left)
            .w(px(232.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.sm)
            .child(
                TextField::new()
                    .icon(IconName::Search)
                    .placeholder("Search nodes"),
            )
            .child(
                div()
                    .px(theme.spacing.xs)
                    .child(Label::caption("Node library")),
            )
            .child(list)
            .child(
                div().px(theme.spacing.xs).child(
                    Label::new("Click a node to add it to the canvas")
                        .size(TextSize::XSmall)
                        .color(TextColor::Faint),
                ),
            )
    }

    fn socket(
        &self,
        node: &Node,
        index: usize,
        output: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        const HIT: f32 = 18.;
        let id = node.id;
        let spec = node.kind.spec();
        let (name, data) = if output {
            spec.outputs[index]
        } else {
            spec.inputs[index]
        };
        let color = data.color(theme);
        let connected = self.links.iter().any(|link| {
            if output {
                link.from == id && link.output == index
            } else {
                link.to == id && link.input == index
            }
        });
        // While a wire is being dragged, inputs it can't join step back.
        let compatible = match self.drag {
            Drag::Wire {
                from, output: o, ..
            } => output || (from != id && self.data_type(from, o, true) == Some(data)),
            _ => true,
        };
        let offset = -(px(1.) + px(HIT / 2.));
        let row = Geometry::new(theme).row;
        let tooltip = SharedString::from(format!("{name} · {}", data.label()));
        div()
            .id((if output { "out" } else { "in" }, id * 16 + index))
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
                        this.input_mouse_down(id, index, event);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                        this.input_mouse_up(id, index);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
            })
    }

    fn widget(name: &'static str, param: Param, theme: &Theme) -> impl IntoElement + use<> {
        let row = Geometry::new(theme).row;
        let well = div()
            .relative()
            .rounded(theme.shape.radius_xs)
            .bg(theme.colors.sunken)
            .overflow_hidden();
        match param {
            Param::Number(value, readout) => well
                .h(row)
                .flex()
                .items_center()
                .justify_between()
                .px(theme.spacing.sm)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(relative(value))
                        .bg(theme.colors.hover),
                )
                .child(
                    Label::new(name)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted),
                )
                .child(Label::new(readout).size(TextSize::XSmall).mono()),
            Param::Choice(options, selected) => well
                .h(row)
                .flex()
                .items_center()
                .gap(theme.spacing.xs)
                .px(theme.spacing.sm)
                .child(
                    Label::new(name)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted),
                )
                .child(div().flex_1())
                .child(Label::new(options[selected]).size(TextSize::XSmall))
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size(px(10.))
                        .color(theme.colors.text_muted),
                ),
            Param::Text(text) => well
                .flex()
                .flex_col()
                .gap(theme.spacing.xxs)
                .p(theme.spacing.sm)
                .child(
                    Label::new(name)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted),
                )
                .child(Label::new(text).size(TextSize::XSmall)),
        }
    }

    fn state_mark(node: &Node, theme: &Theme) -> impl IntoElement + use<> {
        let mark = div().flex().items_center().gap(theme.spacing.xs);
        match node.state {
            NodeState::Idle if node.by_agent => mark.child(
                Icon::new(IconName::Sparkles)
                    .size(px(12.))
                    .color(theme.colors.agent),
            ),
            NodeState::Idle => mark,
            NodeState::Queued => mark.child(
                Label::new("Queued")
                    .size(TextSize::XSmall)
                    .color(TextColor::Faint),
            ),
            NodeState::Running(progress) => mark
                .child(
                    Label::new(format!("{:.0} %", progress * 100.))
                        .size(TextSize::XSmall)
                        .mono()
                        .color(TextColor::Muted),
                )
                .child(
                    Spinner::new(("node-spinner", node.id))
                        .size(px(12.))
                        .color(theme.colors.accent),
                ),
            NodeState::Done(seconds) => mark
                .child(
                    Label::new(format!("{seconds:.1} s"))
                        .size(TextSize::XSmall)
                        .mono()
                        .color(TextColor::Faint),
                )
                .child(
                    Icon::new(IconName::Check)
                        .size(px(12.))
                        .color(theme.colors.success),
                ),
            NodeState::Cached => mark.child(
                Label::new("Cached")
                    .size(TextSize::XSmall)
                    .color(TextColor::Faint),
            ),
        }
    }

    fn preview(node: &Node, theme: &Theme) -> impl IntoElement + use<> {
        let body = div().px(theme.spacing.sm).pb(theme.spacing.sm);
        match node.state {
            NodeState::Done(_) | NodeState::Cached => body.child(
                AudioPlayer::new(("preview", node.id), 0., "0:00", "0:32").peaks(preview_peaks()),
            ),
            NodeState::Queued | NodeState::Running(_) => body.child(
                Skeleton::new(("preview-skeleton", node.id))
                    .w_full()
                    .h(px(44.)),
            ),
            NodeState::Idle if node.has_result => body.child(
                AudioPlayer::new(("preview", node.id), 0., "0:00", "0:32").peaks(preview_peaks()),
            ),
            NodeState::Idle => body.child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme.shape.radius_xs)
                    .bg(theme.colors.sunken)
                    .child(
                        Label::new("Run the workflow to hear the take")
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    ),
            ),
        }
    }

    fn render_node(&self, node: &Node, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let geometry = Geometry::new(&theme);
        let spec = node.kind.spec();
        let id = node.id;
        let selected = self.selected == Some(id);
        let running = matches!(node.state, NodeState::Running(_));
        let dragging = matches!(self.drag, Drag::Node(dragged, _) if dragged == id);
        let rows = spec.inputs.len().max(spec.outputs.len());
        let border = if node.by_agent {
            theme.colors.agent
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
            .child(
                Icon::new(spec.category.icon())
                    .size(px(13.))
                    .color(theme.colors.text_muted),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Label::new(spec.title)
                        .size(TextSize::Small)
                        .weight(theme.typography.label_weight)
                        .truncate(),
                ),
            )
            .child(Self::state_mark(node, &theme));

        let ports = div()
            .flex()
            .flex_col()
            .pt(geometry.pad)
            .children((0..rows).map(|row| {
                let port_label = |name: &'static str| {
                    Label::new(name)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted)
                };
                div()
                    .relative()
                    .h(geometry.row)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(theme.spacing.md)
                    .child(
                        spec.inputs
                            .get(row)
                            .map(|(name, _)| port_label(name).into_any_element())
                            .unwrap_or_else(|| div().into_any_element()),
                    )
                    .child(
                        spec.outputs
                            .get(row)
                            .map(|(name, _)| port_label(name).into_any_element())
                            .unwrap_or_else(|| div().into_any_element()),
                    )
                    .when(row < spec.inputs.len(), |r| {
                        r.child(self.socket(node, row, false, &theme, cx))
                    })
                    .when(row < spec.outputs.len(), |r| {
                        r.child(self.socket(node, row, true, &theme, cx))
                    })
            }));

        let params = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .px(theme.spacing.sm)
            .pt(theme.spacing.xs)
            .pb(theme.spacing.sm)
            .children(
                node.params
                    .iter()
                    .map(|(name, param)| Self::widget(name, *param, &theme)),
            );

        div()
            .id(("node", id))
            .absolute()
            .left(self.pan.x + node.position.x)
            .top(self.pan.y + node.position.y)
            .w(node.kind.width())
            .flex()
            .flex_col()
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(border)
            .when(node.by_agent, |node| node.border_dashed())
            .shadow(if dragging {
                theme.elevation.medium.clone()
            } else {
                theme.elevation.low.clone()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.select(id);
                    this.drag = Drag::Node(id, event.position);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(header)
            .when(running, |node_box| {
                node_box.child(
                    div().h(px(2.)).bg(theme.colors.accent_wash).child(
                        div()
                            .h_full()
                            .w(relative(match node.state {
                                NodeState::Running(progress) => progress,
                                _ => 0.,
                            }))
                            .bg(theme.colors.accent),
                    ),
                )
            })
            .child(ports)
            .when(!node.params.is_empty(), |node_box| node_box.child(params))
            .when(node.kind == Kind::Preview, |node_box| {
                node_box.child(Self::preview(node, &theme))
            })
            .when(node.by_agent, |node_box| {
                node_box.child(
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
                            Label::new("Added by the assistant")
                                .size(TextSize::XSmall)
                                .color(TextColor::Agent),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(theme.spacing.sm)
                                .child(
                                    Button::new(("undo-agent", id), "Undo")
                                        .ghost()
                                        .icon(IconName::Undo)
                                        .size(ControlSize::Small)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.undo_agent_node(id);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(("keep-agent", id), "Keep")
                                        .size(ControlSize::Small)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(node) = this.node_mut(id) {
                                                node.by_agent = false;
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
    }

    /// The dot grid and the wires, painted under the nodes.
    fn wires(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let geometry = Geometry::new(theme);
        let pan = self.pan;
        let highlight = self.selected;
        let wires: Vec<_> = self
            .links
            .iter()
            .filter_map(|link| {
                let from = self.node(link.from)?;
                let to = self.node(link.to)?;
                let data = self.data_type(link.from, link.output, true)?;
                let strong = highlight == Some(link.from) || highlight == Some(link.to);
                Some((
                    pan + geometry.port(from, link.output, true),
                    pan + geometry.port(to, link.input, false),
                    data.color(theme),
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
                let data = self.data_type(from, output, true)?;
                Some((
                    pan + geometry.port(node, output, true),
                    cursor,
                    data.color(theme),
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
            .child(
                Button::new("reset-view", "Reset view")
                    .ghost()
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pan = point(px(16.), px(24.));
                        cx.notify();
                    })),
            )
            .child(Divider::vertical())
            .children(DataType::ALL.into_iter().map(|data| {
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(StatusDot::new(data.color(theme)))
                    .child(
                        Label::new(data.label())
                            .size(TextSize::XSmall)
                            .color(TextColor::Muted),
                    )
            }))
    }

    fn graph(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .map(|node| self.render_node(node, cx).into_any_element())
            .collect();
        div()
            .id("graph")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .bg(theme.colors.background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, _, cx| this.canvas_mouse_down(event, cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event, _, cx| this.canvas_mouse_down(event, cx)),
            )
            .on_mouse_move(cx.listener(|this, event, _, cx| this.canvas_mouse_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.canvas_mouse_up(cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _, _, cx| this.canvas_mouse_up(cx)),
            )
            .on_scroll_wheel(cx.listener(|this, event, _, cx| this.canvas_scroll(event, cx)))
            .child(div().absolute().inset_0().child(self.wires(cx)))
            .children(nodes)
            .child(self.legend(cx))
    }

    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let body = div()
            .id("inspector")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(theme.spacing.xl)
            .p(theme.spacing.lg);

        let selection = match self.selected.and_then(|id| self.node(id)) {
            Some(node) => {
                let spec = node.kind.spec();
                let id = node.id;
                let inputs = spec.inputs.iter().enumerate().map(|(index, (name, data))| {
                    let source = self
                        .links
                        .iter()
                        .find(|link| link.to == id && link.input == index)
                        .and_then(|link| {
                            let from = self.node(link.from)?.kind.spec();
                            Some(format!("{} · {}", from.title, from.outputs[link.output].0))
                        });
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .child(StatusDot::new(data.color(&theme)))
                        .child(
                            div()
                                .w(px(88.))
                                .child(Label::new(*name).size(TextSize::Small)),
                        )
                        .child(
                            div().flex_1().min_w_0().child(match source {
                                Some(source) => Label::new(source)
                                    .size(TextSize::Small)
                                    .color(TextColor::Muted)
                                    .truncate(),
                                None => Label::new("Not connected")
                                    .size(TextSize::Small)
                                    .color(TextColor::Warning),
                            }),
                        )
                });
                let controls = node
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, (name, param))| match *param {
                        Param::Number(value, readout) => Slider::new(value)
                            .label(*name)
                            .readout(readout)
                            .into_any_element(),
                        Param::Choice(options, selected) => div()
                            .flex()
                            .flex_col()
                            .gap(theme.spacing.xs)
                            .child(
                                Label::new(*name)
                                    .size(TextSize::Small)
                                    .color(TextColor::Muted),
                            )
                            .child(
                                SegmentedControl::new(
                                    ("param", id * 16 + index),
                                    options.iter().map(|option| Segment::label(*option)),
                                    selected,
                                )
                                .size(ControlSize::Small)
                                .on_select(cx.listener(
                                    move |this, choice: &usize, _, cx| {
                                        if let Some(node) = this.node_mut(id)
                                            && let Param::Choice(options, _) = node.params[index].1
                                        {
                                            node.params[index].1 = Param::Choice(options, *choice);
                                        }
                                        this.mark_dirty(id);
                                        cx.notify();
                                    },
                                )),
                            )
                            .into_any_element(),
                        Param::Text(text) => div()
                            .flex()
                            .flex_col()
                            .gap(theme.spacing.xs)
                            .child(
                                Label::new(*name)
                                    .size(TextSize::Small)
                                    .color(TextColor::Muted),
                            )
                            .child(TextField::new().value(text))
                            .into_any_element(),
                    });
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.lg)
                    .child(SectionHeader::new(spec.title).description(format!(
                        "{} · node {}",
                        spec.category.label(),
                        id
                    )))
                    .when(node.by_agent, |section| {
                        section.child(
                            Badge::new("Added by the assistant")
                                .tone(Tone::Agent)
                                .icon(IconName::Sparkles),
                        )
                    })
                    .when(!spec.inputs.is_empty(), |section| {
                        section.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(theme.spacing.sm)
                                .child(Label::caption("Inputs"))
                                .children(inputs),
                        )
                    })
                    .when(!node.params.is_empty(), |section| {
                        section.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(theme.spacing.md)
                                .child(Label::caption("Parameters"))
                                .children(controls),
                        )
                    })
                    .child(
                        div().flex().child(
                            Button::new("delete-node", "Delete node")
                                .variant(ButtonVariant::Danger)
                                .icon(IconName::Trash)
                                .size(ControlSize::Small)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_node(id);
                                    cx.notify();
                                })),
                        ),
                    )
                    .into_any_element()
            }
            None => SectionHeader::new("Workflow")
                .description(format!(
                    "{} nodes · {} links. Select a node to see its inputs and parameters.",
                    self.nodes.len(),
                    self.links.len()
                ))
                .into_any_element(),
        };

        let queue = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.sm)
            .child(Label::caption("Queue"))
            .when(self.run.is_some(), |queue| {
                let (done, total) = self.run_progress;
                queue.child(
                    TaskProgress::new("run", format!("Run {}", self.history.len() + 1))
                        .icon(IconName::Play)
                        .detail(format!(
                            "{} · node {} of {}",
                            self.running_title,
                            done + 1,
                            total
                        ))
                        .progress(done as f32 / total.max(1) as f32)
                        .on_cancel(cx.listener(|this, _, _, cx| this.stop_run(cx))),
                )
            })
            .children(self.history.iter().rev().map(|record| {
                ListItem::new(
                    ("run-record", record.number),
                    format!("Run {}", record.number),
                )
                .icon(IconName::Check)
                .icon_color(theme.colors.success)
                .description(if record.reused > 0 {
                    format!("{} nodes reused from cache", record.reused)
                } else {
                    "Every node ran".to_string()
                })
                .trailing(
                    Label::new(format!("{:.1} s", record.seconds))
                        .size(TextSize::XSmall)
                        .mono()
                        .color(TextColor::Faint),
                )
            }));

        Sidebar::new(Edge::Right)
            .w(px(312.))
            .child(body.child(selection).child(queue))
            .child(
                div()
                    .p(theme.spacing.md)
                    .border_t_1()
                    .border_color(theme.colors.separator)
                    .child(
                        AskBar::new("ask", "Ask about this workflow…")
                            .shortcut(if cfg!(target_os = "macos") {
                                "⌘K"
                            } else {
                                "Ctrl+K"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.agent_open = true;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn agent_panel(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let panel = AgentPanel::new("agent", "Match my other tracks' loudness")
            .logo(ProviderLogo::Anthropic)
            .status(AgentStatus::Completed)
            .status_detail("2 tools · 9 s")
            .action(Button::icon_only("history", IconName::History).tooltip("Conversations"))
            .on_close(cx.listener(|this, _, _, cx| {
                this.agent_open = false;
                cx.notify();
            }))
            .entry(
                UserMessage::new(
                    "Takes from this workflow come out much quieter than the rest of my album. Can you fix that in the graph?",
                )
                .id("message-1"),
            )
            .entry(
                ToolGroup::new("Used 2 tools")
                    .call(
                        ToolCall::new(
                            "tool-read",
                            "read_workflow",
                            "Read 10 nodes and 14 links",
                            ToolState::Succeeded,
                        )
                        .icon(IconName::Eye),
                    )
                    .call(
                        ToolCall::new(
                            "tool-add",
                            "add_node",
                            "Added Loudness normalize after Decode audio",
                            ToolState::Succeeded,
                        )
                        .icon(IconName::Plus),
                    ),
            )
            .entry(
                AssistantMessage::new(
                    "Your last three takes measured around −22 LUFS, while the album sits near −14. I added a Loudness normalize node between Decode audio and both outputs:\n\n- Target −14 LUFS integrated, true peak −1 dBTP.\n- Preview and Save now both receive the normalized audio.\n\nIt is marked on the canvas; Undo there rewires Decode audio straight to the outputs.",
                )
                .id("message-2")
                .logo(ProviderLogo::Anthropic)
                .meta("Claude Opus 5.5 · 9 s")
                .action(Button::icon_only("copy", IconName::Copy).size(ControlSize::Small)),
            )
            .composer(
                Composer::new("composer")
                    .placeholder("Ask about the workflow, or describe a change…")
                    .when_some(self.selected.and_then(|id| self.node(id)), |composer, node| {
                        composer.context(ContextChip::new(
                            "ctx-node",
                            node.kind.spec().category.icon(),
                            node.kind.spec().title,
                        ))
                    })
                    .model("Claude Opus 5.5")
                    .model_logo(ProviderLogo::Anthropic),
            );
        div()
            .w(px(width))
            .flex_none()
            .border_l_1()
            .border_color(theme.colors.border)
            .child(panel)
    }
}

/// A fixed, pad-like envelope for the preview player.
fn preview_peaks() -> Vec<f32> {
    (0..64)
        .map(|index| {
            let t = index as f32 / 63.;
            let swell = (t * 4.).min(1.) * (1. - t).powf(0.4);
            let ripple = 0.12 * ((index * 37 % 11) as f32 / 11.);
            (0.15 + 0.7 * swell + ripple).min(1.)
        })
        .collect()
}

/// A wire from an output to an input, leaving and entering horizontally.
fn bezier(from: Point<Pixels>, to: Point<Pixels>, width: Pixels) -> Option<gpui::Path<Pixels>> {
    let reach = ((to.x - from.x).abs() * 0.5).max(px(48.));
    let mut builder = PathBuilder::stroke(width);
    builder.move_to(from);
    builder.cubic_bezier_to(to, point(from.x + reach, from.y), point(to.x - reach, to.y));
    builder.build().ok()
}

impl Render for NodeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        // The node library goes first when the window narrows, and the assistant slims, so the
        // canvas keeps room to work in.
        let wide = window.viewport_size().width >= px(1180.);
        let agent_width = if wide { 380. } else { 320. };
        let running = self.run.is_some();
        let missing = self
            .nodes
            .iter()
            .map(|node| {
                let spec = node.kind.spec();
                (0..spec.inputs.len())
                    .filter(|input| {
                        !self
                            .links
                            .iter()
                            .any(|link| link.to == node.id && link.input == *input)
                    })
                    .count()
            })
            .sum::<usize>();
        let run_button = if running {
            Button::new("stop", "Stop")
                .icon(IconName::Stop)
                .size(ControlSize::Small)
                .on_click(cx.listener(|this, _, _, cx| this.stop_run(cx)))
        } else {
            Button::new("run", "Run")
                .primary()
                .icon(IconName::Play)
                .size(ControlSize::Small)
                // As in ComfyUI, a graph with an empty input doesn't queue.
                .disabled(missing > 0)
                .on_click(cx.listener(|this, _, _, cx| this.start_run(cx)))
        };
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Pad textures")
                    .child(run_button)
                    .when(missing > 0, |bar| {
                        bar.child(
                            Badge::new(if missing == 1 {
                                "1 input not connected".to_string()
                            } else {
                                format!("{missing} inputs not connected")
                            })
                            .tone(Tone::Warning)
                            .icon(IconName::Alert),
                        )
                    })
                    .trailing(switch)
                    .trailing(Divider::vertical())
                    .trailing(
                        AgentToggle::new("agent-toggle", self.agent_open, AgentStatus::Completed)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.agent_open = !this.agent_open;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(wide, |row| row.child(self.library(cx)))
                    .child(self.graph(cx))
                    .map(|row| {
                        if self.agent_open {
                            row.child(self.agent_panel(agent_width, cx).into_any_element())
                        } else {
                            row.child(self.inspector(cx).into_any_element())
                        }
                    }),
            )
    }
}

fn main() {
    common::run("Workflow", 1760., 1000., |choice, _| {
        NodeEditor::new(choice)
    });
}
