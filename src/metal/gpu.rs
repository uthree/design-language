//! The metal on the GPU: each frame is shaded into an off-screen texture by `liquid.wgsl` on the
//! crate's own wgpu device ([`crate::gpu`]), read back, and handed to gpui as an image.
//!
//! Only a hardware adapter is used. Without one, or if anything fails, [`render`] returns `None`
//! and the backdrop falls back to the CPU.

use std::sync::{Mutex, OnceLock};

use super::sim::WAVES;
use super::{DROPS, Studio, drop_shadow, relief};

/// Turns of the ripples per cycle of the backdrop.
const RIPPLE_TURNS: f32 = 6.;

struct Gpu {
    shared: &'static crate::gpu::Gpu,
    pipeline: wgpu::RenderPipeline,
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

/// One frame at `phase` of the cycle, `width` × `height` pixels, BGRA, or `None` if the GPU
/// couldn't shade it. `drops` are each drop's centre and reach, and `waves` each ripple's centre,
/// spread and strength, in pixels.
pub(super) fn render(
    width: u32,
    height: u32,
    phase: f32,
    drops: &[(f32, f32, f32)],
    waves: &[[f32; 4]],
    studio: &Studio,
) -> Option<Vec<u8>> {
    gpu()?.render(
        width,
        height,
        &params(width, height, phase, drops, waves, studio),
    )
}

impl Gpu {
    fn new() -> Option<Self> {
        let shared = crate::gpu::gpu()?;
        // A software rasteriser is slower than the CPU path, which is tuned for it.
        if shared.software && std::env::var_os("ATELIER_METAL_SOFTWARE_GPU").is_none() {
            return None;
        }
        let device = &shared.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("liquid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("liquid.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("liquid"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&layout],
                    push_constant_ranges: &[],
                }),
            ),
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
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("liquid params"),
            size: (PARAMS_LEN * 4) as u64,
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
            pipeline,
            params,
            bindings,
            target: Mutex::new(None),
        })
    }

    fn render(&self, width: u32, height: u32, params: &[f32]) -> Option<Vec<u8>> {
        let shared = self.shared;
        if !shared.fits(width, height) {
            return None;
        }
        shared.queue.write_buffer(
            &self.params,
            0,
            &params
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
                label: Some("liquid"),
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.draw(0..3, 0..1);
        }
        shared.read_back(encoder, &target.texture, &target.readback)
    }

    fn target(&self, width: u32, height: u32) -> Target {
        let texture = self.shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("liquid frame"),
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

/// The shader's `Params`, in 4-byte words.
const PARAMS_LEN: usize = 12 + 3 * 4 + 9 * 4 + 16 * 4 + WAVES * 4;

/// A ripple's wavelength and the width of its band, in units of the box's shorter side, and its
/// height relative to the relief.
const WAVE: (f32, f32, f32) = (0.028, 0.05, 0.009);

/// The shader's `Params` for one frame, laid out as WGSL lays out the struct.
fn params(
    width: u32,
    height: u32,
    phase: f32,
    drops: &[(f32, f32, f32)],
    waves: &[[f32; 4]],
    studio: &Studio,
) -> [f32; PARAMS_LEN] {
    const _: () = assert!(DROPS.len() <= 16);
    let (w, h) = (width as f32, height as f32);
    let unit = w.min(h);
    let (offset, scale) = drop_shadow(unit);
    let mut words = [0f32; PARAMS_LEN];
    words[..12].copy_from_slice(&[
        w,
        h,
        phase * RIPPLE_TURNS * std::f32::consts::TAU,
        relief(unit),
        offset,
        scale,
        studio.shadow,
        f32::from_bits(drops.len().min(16) as u32),
        f32::from_bits(waves.len().min(WAVES) as u32),
        WAVE.0 * unit,
        WAVE.1 * unit,
        WAVE.2,
    ]);
    let colors = [studio.background, studio.behind, studio.front]
        .into_iter()
        .chain(studio.ramp)
        .map(|[r, g, b]| [r, g, b, 0.]);
    let drops = drops.iter().map(|&(x, y, reach)| [x, y, reach, 0.]);
    let slots = words[12..].as_chunks_mut::<4>().0.iter_mut();
    let colors_then_drops = colors.chain(drops.chain(std::iter::repeat([0.; 4])).take(16));
    for (slot, value) in slots.zip(colors_then_drops.chain(waves.iter().copied())) {
        *slot = value;
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Appearance, Direction, Theme};

    /// Writes raw BGRA frames to `$ATELIER_METAL_DUMP` for a look, and times them.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn frames() {
        let dump = std::env::var("ATELIER_METAL_DUMP").ok();
        for (direction, appearance) in [
            (Direction::Glass, Appearance::Dark),
            (Direction::Studio, Appearance::Light),
            (Direction::Mono, Appearance::Light),
        ] {
            let theme = Theme::new(direction, appearance);
            let studio = Studio::new(theme.colors.metal, theme.colors.background, appearance);
            let drops = super::super::drops_at(1300., 750., 0.12);
            // Two ripples on the large pool, upper right, one fresh and one fading.
            let (x, y, _) = drops[0];
            let waves = [[x - 60., y, 50., 1.], [x + 40., y + 90., 160., 0.4]];
            let frame = || render(1300, 750, 0.12, &drops, &waves, &studio);
            let Some(_) = frame() else {
                println!("no GPU");
                return;
            };
            let start = std::time::Instant::now();
            let pixels = frame().unwrap();
            println!(
                "{direction:?} {appearance:?}: {:?} per 1300 x 750 frame",
                start.elapsed()
            );
            if let Some(dump) = &dump {
                std::fs::write(format!("{dump}/{direction:?}-{appearance:?}.bgra"), pixels)
                    .unwrap();
            }
        }
    }
}
