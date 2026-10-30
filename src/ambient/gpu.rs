//! The backdrops on the GPU: each frame is shaded into an off-screen texture by the backdrop's
//! shader (`common.wgsl` and its own file) on the crate's wgpu device ([`crate::gpu`]), read back,
//! and handed to gpui as an image.
//!
//! Only a hardware adapter is used. Without one, or if anything fails, [`render`] returns `None`
//! and the backdrop falls back to the CPU.

use std::sync::{Mutex, OnceLock};

use super::{Ambience, Frame, figures, stars};

struct Gpu {
    shared: &'static crate::gpu::Gpu,
    layout: wgpu::PipelineLayout,
    /// Each backdrop's pipeline, built the first time it is drawn.
    pipelines: [OnceLock<wgpu::RenderPipeline>; Ambience::ALL.len()],
    params: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    target: Mutex<Option<Target>>,
}

/// The texture a frame is shaded into, and the buffer it is read back through, for one size.
struct Target {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    readback: wgpu::Buffer,
}

fn gpu() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(Gpu::new).as_ref()
}

/// Whether frames can be shaded on the GPU.
pub(super) fn available() -> bool {
    gpu().is_some()
}

/// `frame` shaded as `ambience`, BGRA, or `None` if the GPU couldn't shade it.
pub(super) fn render(ambience: Ambience, frame: &Frame) -> Option<Vec<u8>> {
    gpu()?.render(ambience, frame)
}

pub(super) fn source(ambience: Ambience) -> &'static str {
    match ambience {
        Ambience::Aurora => include_str!("aurora.wgsl"),
        Ambience::Dunes => include_str!("dunes.wgsl"),
        Ambience::Bloom => include_str!("bloom.wgsl"),
        Ambience::Caustics => include_str!("caustics.wgsl"),
        Ambience::Solar => include_str!("solar.wgsl"),
        Ambience::Galaxy => include_str!("galaxy.wgsl"),
        Ambience::Jellyfish => include_str!("jellyfish.wgsl"),
        Ambience::Shoal => include_str!("shoal.wgsl"),
        Ambience::Formation => include_str!("formation.wgsl"),
        Ambience::Garden => include_str!("garden.wgsl"),
    }
}

impl Gpu {
    fn new() -> Option<Self> {
        let shared = crate::gpu::gpu()?;
        // A software rasteriser is slower than the CPU path, which is tuned for it.
        if shared.software && std::env::var_os("ATELIER_AMBIENT_SOFTWARE_GPU").is_none() {
            return None;
        }
        let device = &shared.device;
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ambient params"),
            size: (PARAMS_LEN * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            }],
        });
        Some(Self {
            shared,
            layout,
            pipelines: Default::default(),
            params,
            bindings,
            target: Mutex::new(None),
        })
    }

    fn pipeline(&self, ambience: Ambience) -> &wgpu::RenderPipeline {
        self.pipelines[ambience as usize].get_or_init(|| {
            let device = &self.shared.device;
            let source = [include_str!("common.wgsl"), source(ambience)].concat();
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(ambience.label()),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(ambience.label()),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(crate::gpu::FORMAT.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        })
    }

    fn render(&self, ambience: Ambience, frame: &Frame) -> Option<Vec<u8>> {
        let shared = self.shared;
        let (width, height) = (frame.width, frame.height);
        if !shared.fits(width, height) {
            return None;
        }
        let pipeline = self.pipeline(ambience);
        shared.queue.write_buffer(
            &self.params,
            0,
            &params(frame)
                .iter()
                .flat_map(|value| value.to_ne_bytes())
                .collect::<Vec<_>>(),
        );

        let mut slot = self.target.lock().ok()?;
        if !matches!(&*slot, Some(target) if target.width == width && target.height == height) {
            *slot = Some(self.target(width, height));
        }
        let target = slot.as_ref()?;
        let view = target.texture.create_view(&Default::default());
        let mut encoder = shared.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(ambience.label()),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.draw(0..3, 0..1);
        }
        shared.read_back(encoder, &target.texture, &target.readback)
    }

    fn target(&self, width: u32, height: u32) -> Target {
        let texture = self.shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ambient frame"),
            size: crate::gpu::extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::gpu::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Target {
            width,
            height,
            texture,
            readback: self.shared.readback(width, height),
        }
    }
}

/// Where Aurora's sky starts in the shader's `Params`, in 4-byte words: after the frame, the
/// colours and the ripples.
pub(super) const SKY: usize = 12 + 4 + 4 + 4 * 4 + super::sim::WAVES * 4;
/// Where the stars start: after the two counts, padded to a whole vector.
pub(super) const STARS: usize = SKY + 4;
pub(super) const LINES: usize = STARS + stars::STARS * 4;
pub(super) const STRENGTHS: usize = LINES + stars::LINES * 4;
pub(super) const METEORS: usize = STRENGTHS + stars::LINES;
/// Where Formation's and Garden's pieces start: after their four counts.
pub(super) const COUNTS: usize = METEORS + stars::METEORS * 4;
pub(super) const FIGURES: usize = COUNTS + 4;
/// The shader's `Params`, in 4-byte words.
pub(super) const PARAMS_LEN: usize = FIGURES + figures::FIGURES * 8;

/// The shader's `Params` for one frame, laid out as WGSL lays out the struct.
fn params(frame: &Frame) -> [f32; PARAMS_LEN] {
    let response = &frame.response;
    let palette = &frame.palette;
    let mut words = [0f32; PARAMS_LEN];
    words[..12].copy_from_slice(&[
        frame.width as f32,
        frame.height as f32,
        frame.time,
        if palette.dark { 1. } else { 0. },
        response.pointer[0],
        response.pointer[1],
        response.presence,
        f32::from_bits(response.wave_count as u32),
        response.velocity[0],
        response.velocity[1],
        response.sway[0],
        response.sway[1],
    ]);
    let colors = [palette.base, palette.deep]
        .into_iter()
        .chain(palette.hues)
        .map(|[r, g, b]| [r, g, b, 0.]);
    let slots = words[12..].as_chunks_mut::<4>().0.iter_mut();
    for (slot, value) in slots.zip(colors.chain(response.waves)) {
        *slot = value;
    }
    let sky = &frame.sky;
    words[SKY] = f32::from_bits(sky.line_count as u32);
    words[SKY + 1] = f32::from_bits(sky.meteor_count as u32);
    words[STARS..LINES].copy_from_slice(sky.stars.as_flattened());
    words[LINES..STRENGTHS].copy_from_slice(sky.lines.as_flattened());
    words[STRENGTHS..METEORS].copy_from_slice(&sky.strengths);
    words[METEORS..COUNTS].copy_from_slice(sky.meteors.as_flattened());
    for (word, count) in words[COUNTS..FIGURES].iter_mut().zip(frame.figures.counts) {
        *word = f32::from_bits(count);
    }
    words[FIGURES..].copy_from_slice(frame.figures.items.as_flattened().as_flattened());
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Appearance, Direction, Theme};

    /// Writes raw BGRA frames to `$ATELIER_AMBIENT_DUMP` for a look, and times them.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn frames() {
        let dump = std::env::var("ATELIER_AMBIENT_DUMP").ok();
        for ambience in Ambience::ALL {
            for (direction, appearance) in [
                (Direction::Glass, Appearance::Dark),
                (Direction::Glass, Appearance::Light),
            ] {
                let theme = Theme::new(direction, appearance);
                let phase = std::env::var("ATELIER_AMBIENT_PHASE")
                    .ok()
                    .and_then(|phase| phase.parse().ok())
                    .unwrap_or(0.12);
                let frame = Frame::still(ambience, 1300, 750, phase, &theme);
                let Some(_) = render(ambience, &frame) else {
                    println!("no GPU");
                    return;
                };
                let start = std::time::Instant::now();
                let pixels = render(ambience, &frame).unwrap();
                println!(
                    "{ambience:?} {direction:?} {appearance:?}: {:?} per 1300 x 750 frame",
                    start.elapsed()
                );
                if let Some(dump) = &dump {
                    std::fs::write(
                        format!("{dump}/{ambience:?}-{direction:?}-{appearance:?}.bgra"),
                        pixels,
                    )
                    .unwrap();
                }
            }
        }
    }

    #[test]
    fn every_shader_compiles() {
        for ambience in Ambience::ALL {
            let source = [include_str!("common.wgsl"), source(ambience)].concat();
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|error| panic!("{ambience:?}: {}", error.emit_to_string(&source)));
            naga::valid::Validator::new(Default::default(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{ambience:?}: {error:?}"));
        }
    }
}
