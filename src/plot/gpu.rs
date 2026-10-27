//! A [`super::Plot3d`]'s contents on the GPU: the surface, curves and points are shaded off
//! screen by `plot3d.wgsl` on the crate's wgpu device ([`crate::gpu`]), at the display's own
//! resolution with four samples a pixel and a depth buffer, read back and handed to gpui as one
//! image. The walls behind and the labels in front stay with gpui.
//!
//! On the GPU the surface is lit per pixel with a soft sheen, its colours run smoothly across
//! each facet, and its edges are drawn from the grid in the shader, so they stay one width at
//! any angle; curves and points hide behind the surface where it is in front of them, markers
//! are small lit beads, and particles glow (adding up on dark themes). If [`render`] returns
//! `None` the plot draws itself with gpui paths instead.

use std::ops::Range;
use std::sync::{Mutex, OnceLock};

use wgpu::util::DeviceExt;

use crate::gpu::{DEPTH_FORMAT, FORMAT, extent};

/// The shader's `Params`, in 4-byte words.
pub(super) const PARAMS_LEN: usize = 24;

/// Words per surface vertex: position, normal, colour and grid place.
pub(super) const SURFACE_VERTEX: usize = 12;
/// Words per curve segment: each end and its width, then each end's colour.
pub(super) const SEGMENT: usize = 16;
/// Words per point: centre and radius, then colour.
pub(super) const POINT: usize = 8;

/// One frame: `width` × `height` device pixels and what goes in it.
pub(super) struct Frame {
    pub width: u32,
    pub height: u32,
    pub params: [f32; PARAMS_LEN],
    /// The surface's vertices and triangles.
    pub surface: Option<(Vec<f32>, Vec<u32>)>,
    pub segments: Vec<f32>,
    pub points: Vec<f32>,
    /// What to draw after the surface, in order.
    pub draws: Vec<Draw>,
    /// Whether particles add their light up, as on a dark background.
    pub additive: bool,
}

/// A run of segments or points, by index into [`Frame::segments`] or [`Frame::points`].
pub(super) enum Draw {
    Curve(Range<u32>),
    Markers(Range<u32>),
    Particles(Range<u32>),
}

struct Pipelines {
    shared: &'static crate::gpu::Gpu,
    surface: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    marker: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    glow_additive: wgpu::RenderPipeline,
    params: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    target: Mutex<Option<Target>>,
}

/// The textures one frame size is shaded into, and the buffer it is read back through.
struct Target {
    width: u32,
    height: u32,
    /// The multisampled colour, when there is more than one sample a pixel.
    samples: Option<wgpu::Texture>,
    depth: wgpu::Texture,
    resolved: wgpu::Texture,
    readback: wgpu::Buffer,
}

fn pipelines() -> Option<&'static Pipelines> {
    static PIPELINES: OnceLock<Option<Pipelines>> = OnceLock::new();
    PIPELINES.get_or_init(Pipelines::new).as_ref()
}

/// Whether plots can be shaded on the GPU.
pub(super) fn available() -> bool {
    pipelines().is_some()
}

/// The largest frame side the GPU takes.
pub(super) fn fits(width: u32, height: u32) -> bool {
    pipelines().is_some_and(|pipelines| pipelines.shared.fits(width, height))
}

/// `frame`'s pixels, BGRA with straight alpha, or `None` if the GPU couldn't shade it.
pub(super) fn render(frame: &Frame) -> Option<Vec<u8>> {
    pipelines()?.render(frame)
}

fn attributes<const N: usize>(formats: [wgpu::VertexFormat; N]) -> [wgpu::VertexAttribute; N] {
    let mut offset = 0;
    let mut location = 0;
    formats.map(|format| {
        let attribute = wgpu::VertexAttribute {
            format,
            offset,
            shader_location: location,
        };
        offset += format.size();
        location += 1;
        attribute
    })
}

impl Pipelines {
    fn new() -> Option<Self> {
        let shared = crate::gpu::gpu()?;
        let device = &shared.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("plot3d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("plot3d.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        use wgpu::VertexFormat::{Float32x2, Float32x3, Float32x4};
        let surface_attributes = attributes([Float32x3, Float32x3, Float32x4, Float32x2]);
        let segment_attributes = attributes([Float32x4, Float32x4, Float32x4, Float32x4]);
        let point_attributes = attributes([Float32x4, Float32x4]);
        let premultiplied = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: premultiplied.alpha,
        };
        // The surface writes depth; what follows only tests against it, and blends in order.
        let depth = |write: bool| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let pipeline = |label: &str,
                        vertex: &str,
                        fragment: &str,
                        buffer: wgpu::VertexBufferLayout,
                        blend: Option<wgpu::BlendState>,
                        depth_stencil: wgpu::DepthStencilState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &[buffer],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(depth_stencil),
                multisample: wgpu::MultisampleState {
                    count: shared.samples,
                    ..Default::default()
                },
                multiview: None,
                cache: None,
            })
        };
        let words = |count: usize| (count * 4) as wgpu::BufferAddress;
        let instances = |attributes| wgpu::VertexBufferLayout {
            array_stride: 0,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes,
        };
        let surface = pipeline(
            "plot3d surface",
            "vs_surface",
            "fs_surface",
            wgpu::VertexBufferLayout {
                array_stride: words(SURFACE_VERTEX),
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &surface_attributes,
            },
            None,
            depth(true),
        );
        let line = pipeline(
            "plot3d curve",
            "vs_line",
            "fs_line",
            wgpu::VertexBufferLayout {
                array_stride: words(SEGMENT),
                ..instances(&segment_attributes)
            },
            Some(premultiplied),
            depth(false),
        );
        let points = |label, vertex, fragment, blend| {
            pipeline(
                label,
                vertex,
                fragment,
                wgpu::VertexBufferLayout {
                    array_stride: words(POINT),
                    ..instances(&point_attributes)
                },
                Some(blend),
                depth(false),
            )
        };
        let marker = points("plot3d markers", "vs_marker", "fs_marker", premultiplied);
        let glow = points(
            "plot3d particles",
            "vs_particle",
            "fs_particle",
            premultiplied,
        );
        let glow_additive = points("plot3d particles", "vs_particle", "fs_particle", additive);

        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("plot3d params"),
            size: words(PARAMS_LEN),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            }],
        });
        Some(Self {
            shared,
            surface,
            line,
            marker,
            glow,
            glow_additive,
            params,
            bindings,
            target: Mutex::new(None),
        })
    }

    fn render(&self, frame: &Frame) -> Option<Vec<u8>> {
        let (width, height) = (frame.width, frame.height);
        let shared = self.shared;
        let device = &shared.device;
        if width == 0 || height == 0 || !shared.fits(width, height) {
            return None;
        }
        shared
            .queue
            .write_buffer(&self.params, 0, &bytes(&frame.params));
        let buffer = |label, words: &[f32], usage| {
            (!words.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: &bytes(words),
                    usage,
                })
            })
        };
        let vertex = wgpu::BufferUsages::VERTEX;
        let surface = frame.surface.as_ref().and_then(|(vertices, indices)| {
            let indices: Vec<u8> = indices
                .iter()
                .flat_map(|index| index.to_ne_bytes())
                .collect();
            Some((
                buffer("plot3d surface", vertices, vertex)?,
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("plot3d triangles"),
                    contents: &indices,
                    usage: wgpu::BufferUsages::INDEX,
                }),
                (indices.len() / 4) as u32,
            ))
        });
        let segments = buffer("plot3d segments", &frame.segments, vertex);
        let points = buffer("plot3d points", &frame.points, vertex);

        let mut slot = self.target.lock().ok()?;
        if !matches!(&*slot, Some(target) if target.width == width && target.height == height) {
            *slot = Some(self.target(width, height));
        }
        let target = slot.as_ref()?;
        let resolved = target.resolved.create_view(&Default::default());
        let samples = target
            .samples
            .as_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let depth = target.depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plot3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: samples.as_ref().unwrap_or(&resolved),
                    resolve_target: samples.as_ref().map(|_| &resolved),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        // With more than one sample a pixel only the resolved frame is kept.
                        store: if samples.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.bindings, &[]);
            if let Some((vertices, indices, count)) = &surface {
                pass.set_pipeline(&self.surface);
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
            for draw in &frame.draws {
                let (pipeline, buffer, range) = match draw {
                    Draw::Curve(range) => (&self.line, &segments, range),
                    Draw::Markers(range) => (&self.marker, &points, range),
                    Draw::Particles(range) if frame.additive => {
                        (&self.glow_additive, &points, range)
                    }
                    Draw::Particles(range) => (&self.glow, &points, range),
                };
                let Some(buffer) = buffer else { continue };
                if range.is_empty() {
                    continue;
                }
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, range.clone());
            }
        }
        let mut pixels = shared.read_back(encoder, &target.resolved, &target.readback)?;
        straighten(&mut pixels);
        Some(pixels)
    }

    fn target(&self, width: u32, height: u32) -> Target {
        let device = &self.shared.device;
        let texture = |label, format, sample_count, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let attachment = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let samples = self.shared.samples;
        Target {
            width,
            height,
            samples: (samples > 1).then(|| texture("plot3d samples", FORMAT, samples, attachment)),
            depth: texture("plot3d depth", DEPTH_FORMAT, samples, attachment),
            resolved: texture(
                "plot3d frame",
                FORMAT,
                1,
                attachment | wgpu::TextureUsages::COPY_SRC,
            ),
            readback: self.shared.readback(width, height),
        }
    }
}

fn bytes(words: &[f32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_ne_bytes()).collect()
}

/// Premultiplied BGRA to straight alpha, which gpui's images hold.
fn straighten(pixels: &mut [u8]) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as u32;
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }
}
