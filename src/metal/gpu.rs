//! The metal on the GPU. gpui can't take a custom shader, so the backdrop has its own small
//! wgpu device beside gpui's renderer: each frame is shaded into an off-screen texture by
//! `liquid.wgsl`, read back, and handed to gpui as an image like any other.
//!
//! Only a hardware adapter is used. Without one, or if anything fails, [`render`] returns `None`
//! and the backdrop falls back to the CPU.

use std::sync::{Mutex, OnceLock};

use super::{DROPS, Studio, drop_shadow, drops_at, relief};

/// Turns of the ripples per cycle of the backdrop.
const RIPPLE_TURNS: f32 = 6.;

/// Rows of the read-back buffer start on multiples of this many bytes.
const ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
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
    GPU.get_or_init(|| pollster::block_on(Gpu::new())).as_ref()
}

/// Whether frames can be shaded on the GPU.
pub(super) fn available() -> bool {
    gpu().is_some()
}

/// One frame at `phase` of the cycle, `width` × `height` pixels, BGRA, or `None` if the GPU
/// couldn't shade it.
pub(super) fn render(width: u32, height: u32, phase: f32, studio: &Studio) -> Option<Vec<u8>> {
    gpu()?.render(width, height, phase, studio)
}

impl Gpu {
    async fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .ok()?;
        // A software rasteriser is slower than the CPU path, which is tuned for it.
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu
            && std::env::var_os("ATELIER_METAL_SOFTWARE_GPU").is_none()
        {
            return None;
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("atelier metal"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .ok()?;

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
                targets: &[Some(wgpu::TextureFormat::Bgra8Unorm.into())],
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
            device,
            queue,
            pipeline,
            params,
            bindings,
            target: Mutex::new(None),
        })
    }

    fn render(&self, width: u32, height: u32, phase: f32, studio: &Studio) -> Option<Vec<u8>> {
        let largest = self.device.limits().max_texture_dimension_2d;
        if width > largest || height > largest {
            return None;
        }
        self.queue.write_buffer(
            &self.params,
            0,
            &params(width, height, phase, studio)
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
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
        let row = padded_row(width);
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            extent(width, height),
        );
        self.queue.submit([encoder.finish()]);

        let slice = target.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        self.device.poll(wgpu::PollType::Wait).ok()?;
        receiver.recv().ok()?.ok()?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        {
            let mapped = slice.get_mapped_range();
            for line in mapped.chunks(row as usize) {
                pixels.extend_from_slice(&line[..(width * 4) as usize]);
            }
        }
        target.readback.unmap();
        Some(pixels)
    }

    fn target(&self, width: u32, height: u32) -> Target {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("liquid frame"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("liquid readback"),
            size: (padded_row(width) * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Target {
            width,
            height,
            texture,
            readback,
        }
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn padded_row(width: u32) -> u32 {
    (width * 4).div_ceil(ROW_ALIGNMENT) * ROW_ALIGNMENT
}

/// The shader's `Params`, in 4-byte words.
const PARAMS_LEN: usize = 8 + 3 * 4 + 9 * 4 + 16 * 4;

/// The shader's `Params` for one frame, laid out as WGSL lays out the struct.
fn params(width: u32, height: u32, phase: f32, studio: &Studio) -> [f32; PARAMS_LEN] {
    const _: () = assert!(DROPS.len() <= 16);
    let (w, h) = (width as f32, height as f32);
    let unit = w.min(h);
    let (offset, scale) = drop_shadow(unit);
    let mut words = [0f32; PARAMS_LEN];
    words[..7].copy_from_slice(&[
        w,
        h,
        phase * RIPPLE_TURNS * std::f32::consts::TAU,
        relief(unit),
        offset,
        scale,
        studio.shadow,
    ]);
    words[7] = f32::from_bits(DROPS.len() as u32);
    let colors = [studio.background, studio.behind, studio.front]
        .into_iter()
        .chain(studio.ramp);
    for (slot, color) in words[8..8 + 12 * 4].chunks_exact_mut(4).zip(colors) {
        slot[..3].copy_from_slice(&color);
    }
    for (slot, (x, y, reach)) in words[8 + 12 * 4..]
        .chunks_exact_mut(4)
        .zip(drops_at(w, h, phase))
    {
        slot[..3].copy_from_slice(&[x, y, reach]);
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
            let Some(_) = render(1300, 750, 0.12, &studio) else {
                println!("no GPU");
                return;
            };
            let start = std::time::Instant::now();
            let pixels = render(1300, 750, 0.12, &studio).unwrap();
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
