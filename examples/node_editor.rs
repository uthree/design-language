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
use atelier::graph::{GraphNode, NodeCanvas, NodeCanvasEvent, NodeField, NodeId, NodeState, Wire};
use atelier::{
    ActiveTheme, Appearance, AudioPlayer, Badge, Button, ButtonVariant, ControlSize, Direction,
    Disclosure, Divider, Edge, IconName, Label, ListItem, ProviderLogo, SectionHeader, Segment,
    SegmentedControl, Sidebar, Skeleton, Slider, StatusDot, TaskProgress, TextColor, TextField,
    TextSize, Theme, TitleBar, Tone, WindowFrame,
};
use common::Themed;
use gpui::{
    AnyElement, App, Context, Entity, Hsla, IntoElement, ParentElement, Pixels, Render,
    SharedString, Styled, Task, Window, div, prelude::*, px,
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
    /// A number shown as a filled bar: its position in the range, from 0 to 1, and its unit.
    Number(f32, Unit),
    /// One of a few options.
    Choice(&'static [&'static str], usize),
    /// Free text, such as a prompt.
    Text(&'static str),
}

/// How a number parameter maps its position, from 0 to 1, to the value it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Bpm,
    Duration,
    Batch,
    Seed,
    Steps,
    Guidance,
    Lufs,
    TruePeak,
    Percent,
}

impl Unit {
    fn format(self, value: f32) -> SharedString {
        match self {
            Unit::Bpm => format!("{:.0} BPM", value * 200.),
            Unit::Duration => {
                let seconds = (value * 128.).round() as u32;
                format!("{}:{:02}", seconds / 60, seconds % 60)
            }
            Unit::Batch => format!("{:.0}", 1. + (value * 4.).round()),
            Unit::Seed => {
                let seed = (value * 1_004_402.).round() as u32;
                format!("{} {:03}", seed / 1000, seed % 1000)
            }
            Unit::Steps => format!("{:.0}", value * 100.),
            Unit::Guidance => format!("{:.1}", value * 10.),
            Unit::Lufs => format!("{:.0} LUFS", value * 35. - 35.).replace('-', "−"),
            Unit::TruePeak => format!("{:.1} dBTP", value * 5. - 5.).replace('-', "−"),
            Unit::Percent => format!("{:.0} %", value * 100.),
        }
        .into()
    }
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
                    ("Tempo", Param::Number(0.42, Unit::Bpm)),
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
                    ("Length", Param::Number(0.25, Unit::Duration)),
                    ("Batch", Param::Number(0.25, Unit::Batch)),
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
                    ("Seed", Param::Number(0.48, Unit::Seed)),
                    ("Steps", Param::Number(0.3, Unit::Steps)),
                    ("Guidance", Param::Number(0.45, Unit::Guidance)),
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
                    ("Target", Param::Number(0.6, Unit::Lufs)),
                    ("True peak", Param::Number(0.8, Unit::TruePeak)),
                ],
                work: 3,
            },
            Self::Reverb => Spec {
                title: "Reverb",
                category: Category::Audio,
                inputs: &[("Audio", Audio)],
                outputs: &[("Audio", Audio)],
                params: &[
                    ("Size", Param::Number(0.7, Unit::Percent)),
                    ("Mix", Param::Number(0.25, Unit::Percent)),
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

/// What the application knows about a node that the canvas does not: what it is and whether
/// its last result can be reused.
struct AppNode {
    kind: Kind,
    params: Vec<(&'static str, Param)>,
    /// Has produced a result that a later run could reuse.
    has_result: bool,
    /// A parameter or an input changed since the last run.
    dirty: bool,
}

/// A finished run, for the queue history.
struct RunRecord {
    number: usize,
    seconds: f32,
    reused: usize,
}

struct NodeEditor {
    choice: (Direction, Appearance),
    canvas: Entity<NodeCanvas>,
    nodes: Vec<(NodeId, AppNode)>,
    selected: Option<NodeId>,
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

/// A parameter as a row on the node's card.
fn field(name: &'static str, param: Param) -> NodeField {
    match param {
        Param::Number(value, unit) => NodeField::number(name, unit.format(value), value),
        Param::Choice(options, selected) => NodeField::choice(name, options[selected]),
        Param::Text(text) => NodeField::text(name, text),
    }
}

/// The card for a kind of node, at a place on the canvas.
fn graph_node(kind: Kind, params: &[(&'static str, Param)], x: f32, y: f32) -> GraphNode {
    let spec = kind.spec();
    let mut node = GraphNode::new(spec.title)
        .icon(spec.category.icon())
        .width(kind.width())
        .at(x, y);
    for (name, data) in spec.inputs {
        node = node.input(*name, *data as usize);
    }
    for (name, data) in spec.outputs {
        node = node.output(*name, *data as usize);
    }
    for (name, param) in params {
        node = node.field(field(name, *param));
    }
    if kind == Kind::Preview {
        node = node.body(preview(NodeState::Idle, false));
    }
    node
}

impl NodeEditor {
    fn new(choice: (Direction, Appearance), cx: &mut Context<Self>) -> Self {
        let canvas =
            cx.new(|_| NodeCanvas::new("graph").port_types(DataType::ALL.map(DataType::label)));
        let layout = [
            (Kind::LoadModel, 0., 20.),
            (Kind::TempoKey, 0., 250.),
            (Kind::Prompt, 228., 0.),
            (Kind::NegativePrompt, 228., 236.),
            (Kind::EmptyLatent, 228., 410.),
            (Kind::Sampler, 476., 60.),
            (Kind::Decode, 724., 60.),
            (Kind::Normalize, 724., 200.),
            (Kind::Preview, 952., 30.),
            (Kind::Save, 952., 250.),
        ];
        let mut nodes = Vec::new();
        canvas.update(cx, |canvas, cx| {
            let ids: Vec<NodeId> = layout
                .iter()
                .map(|(kind, x, y)| {
                    let params = kind.spec().params.to_vec();
                    // The assistant added the normalizer; it waits to be kept.
                    let node =
                        graph_node(*kind, &params, *x, *y).by_agent(*kind == Kind::Normalize);
                    let id = canvas.add(node, cx);
                    nodes.push((
                        id,
                        AppNode {
                            kind: *kind,
                            params,
                            has_result: false,
                            dirty: false,
                        },
                    ));
                    id
                })
                .collect();
            for (from, output, to, input) in [
                (0, 1, 2, 0),
                (1, 0, 2, 1),
                (1, 1, 2, 2),
                (0, 1, 3, 0),
                (1, 0, 4, 0),
                (0, 0, 5, 0),
                (2, 0, 5, 1),
                (3, 0, 5, 2),
                (4, 0, 5, 3),
                (5, 0, 6, 0),
                (0, 2, 6, 1),
                (6, 0, 7, 0),
                (7, 0, 8, 0),
                (7, 0, 9, 0),
            ] {
                canvas.connect(Wire::new(ids[from], output, ids[to], input), cx);
            }
            canvas.select(Some(ids[5]), cx);
        });
        cx.subscribe(&canvas, |this, _, event: &NodeCanvasEvent, cx| {
            this.canvas_event(event, cx)
        })
        .detach();
        let selected = canvas.read(cx).selected();
        Self {
            choice,
            canvas,
            nodes,
            selected,
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

    fn node(&self, id: NodeId) -> Option<&AppNode> {
        self.nodes
            .iter()
            .find(|(node, _)| *node == id)
            .map(|(_, node)| node)
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut AppNode> {
        self.nodes
            .iter_mut()
            .find(|(node, _)| *node == id)
            .map(|(_, node)| node)
    }

    /// Keep the application's model in step with what the person did on the canvas.
    fn canvas_event(&mut self, event: &NodeCanvasEvent, cx: &mut Context<Self>) {
        match event {
            NodeCanvasEvent::Selected(id) => self.selected = *id,
            NodeCanvasEvent::Connected(wire) | NodeCanvasEvent::Disconnected(wire) => {
                self.mark_dirty(wire.to, cx)
            }
            NodeCanvasEvent::Removed(id) | NodeCanvasEvent::Undone(id) => {
                self.nodes.retain(|(node, _)| node != id);
                if self.selected == Some(*id) {
                    self.selected = None;
                }
            }
            NodeCanvasEvent::Moved(_) | NodeCanvasEvent::Kept(_) => {}
        }
        cx.notify();
    }

    // Graph edits.

    fn add_node(&mut self, kind: Kind, cx: &mut Context<Self>) {
        let params = kind.spec().params.to_vec();
        let step = (self.nodes.len() % 6) as f32 * 24.;
        let id = self.canvas.update(cx, |canvas, cx| {
            // Near the top-left of the visible canvas, stepped so new nodes don't stack exactly.
            let pan = canvas.pan();
            let x = 160. + step - f32::from(pan.x);
            let y = 80. + step - f32::from(pan.y);
            let id = canvas.add(graph_node(kind, &params, x, y), cx);
            canvas.select(Some(id), cx);
            id
        });
        self.nodes.push((
            id,
            AppNode {
                kind,
                params,
                has_result: false,
                dirty: false,
            },
        ));
        self.selected = Some(id);
    }

    fn remove_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, cx| canvas.remove(id, cx));
        self.nodes.retain(|(node, _)| *node != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
    }

    /// A changed node, and everything downstream of it, has to run again.
    fn mark_dirty(&mut self, id: NodeId, cx: &mut Context<Self>) {
        for id in self.canvas.read(cx).downstream(id) {
            if let Some(node) = self.node_mut(id) {
                node.dirty = true;
            }
        }
    }

    /// Change a parameter, on the card as well.
    fn set_param(&mut self, id: NodeId, index: usize, param: Param, cx: &mut Context<Self>) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        node.params[index].1 = param;
        let row = field(node.params[index].0, param);
        self.canvas.update(cx, |canvas, cx| {
            canvas.update_node(id, |node| node.fields[index] = row, cx)
        });
        self.mark_dirty(id, cx);
    }

    /// Show a node's place in the run, and for the preview, what it can play.
    fn set_state(&self, id: NodeId, state: NodeState, cx: &mut Context<Self>) {
        let preview_result = self
            .node(id)
            .filter(|node| node.kind == Kind::Preview)
            .map(|node| node.has_result);
        self.canvas.update(cx, |canvas, cx| {
            canvas.update_node(
                id,
                |node| {
                    node.state = state;
                    if let Some(has_result) = preview_result {
                        *node = node.clone().body(preview(state, has_result));
                    }
                },
                cx,
            )
        });
    }

    // Running.

    fn start_run(&mut self, cx: &mut Context<Self>) {
        let order = self.canvas.read(cx).execution_order();
        for id in &order {
            self.set_state(*id, NodeState::Queued, cx);
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
                    let Some(node) = this.node(id) else {
                        return 0;
                    };
                    let spec = node.kind.spec();
                    let reuse = node.has_result && !node.dirty && node.kind != Kind::Sampler;
                    this.running_title = spec.title;
                    this.set_state(
                        id,
                        if reuse {
                            NodeState::Cached
                        } else {
                            NodeState::Running(0.)
                        },
                        cx,
                    );
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
                            this.set_state(id, NodeState::Running(progress), cx)
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
                        node.has_result = true;
                        node.dirty = false;
                    }
                    this.set_state(id, NodeState::Done(Some(took)), cx);
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
        let unfinished: Vec<NodeId> = self
            .canvas
            .read(cx)
            .nodes()
            .filter(|(_, node)| matches!(node.state, NodeState::Queued | NodeState::Running(_)))
            .map(|(id, _)| id)
            .collect();
        for id in unfinished {
            self.set_state(id, NodeState::Idle, cx);
        }
        cx.notify();
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
                let count = self
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.kind == kind)
                    .count();
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
                            this.add_node(kind, cx);
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

        let graph = self.canvas.read(cx);
        let wire_count = graph.wires().len();
        let node_by_agent = self
            .selected
            .and_then(|id| graph.node(id))
            .is_some_and(|node| node.by_agent);
        let sources: Vec<Option<(NodeId, usize)>> = match self.selected {
            Some(id) => (0..16).map(|input| graph.source(id, input)).collect(),
            None => Vec::new(),
        };
        let selection = match self.selected.and_then(|id| Some((id, self.node(id)?))) {
            Some((id, node)) => {
                let spec = node.kind.spec();
                let inputs = spec.inputs.iter().enumerate().map(|(index, (name, data))| {
                    let source = sources[index].and_then(|(from, output)| {
                        let from = self.node(from)?.kind.spec();
                        Some(format!("{} · {}", from.title, from.outputs[output].0))
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
                        Param::Number(value, unit) => Slider::new(value)
                            .id(("param", id.0 as usize * 16 + index))
                            .label(*name)
                            .readout(unit.format(value))
                            .on_change(cx.listener(move |this, value: &f32, _, cx| {
                                this.set_param(id, index, Param::Number(*value, unit), cx);
                                cx.notify();
                            }))
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
                                    ("param", id.0 as usize * 16 + index),
                                    options.iter().map(|option| Segment::label(*option)),
                                    selected,
                                )
                                .size(ControlSize::Small)
                                .on_select(cx.listener(
                                    move |this, choice: &usize, _, cx| {
                                        this.set_param(
                                            id,
                                            index,
                                            Param::Choice(options, *choice),
                                            cx,
                                        );
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
                        id.0
                    )))
                    .when(node_by_agent, |section| {
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
                                    this.remove_node(id, cx);
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
                    wire_count
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

/// The preview node's player, or what stands in for it before the take exists.
fn preview(
    state: NodeState,
    has_result: bool,
) -> impl Fn(NodeId, &mut Window, &mut App) -> AnyElement + 'static {
    move |id, _, cx| {
        let theme = cx.theme();
        match state {
            NodeState::Done(_) | NodeState::Cached => {
                AudioPlayer::new(("preview", id.0 as usize), 0., "0:00", "0:32")
                    .peaks(preview_peaks())
                    .into_any_element()
            }
            NodeState::Queued | NodeState::Running(_) => {
                Skeleton::new(("preview-skeleton", id.0 as usize))
                    .w_full()
                    .h(px(44.))
                    .into_any_element()
            }
            _ if has_result => AudioPlayer::new(("preview", id.0 as usize), 0., "0:00", "0:32")
                .peaks(preview_peaks())
                .into_any_element(),
            _ => div()
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
                )
                .into_any_element(),
        }
    }
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
        let missing = self.canvas.read(cx).unconnected_inputs();
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
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.canvas.clone()),
                    )
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
    common::run("Workflow", 1760., 1000., NodeEditor::new);
}
