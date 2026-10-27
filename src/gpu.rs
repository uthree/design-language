//! The crate's own small wgpu device, beside gpui's renderer. gpui takes no custom shaders, so
//! what needs one — the liquid metal, the 3-D plots — shades off screen here, reads the frame
//! back and hands it to gpui as an image like any other.
//!
//! The device is opened once, on first use, and shared. Without an adapter, or with
//! `ATELIER_GPU=off` in the environment, there is none and every caller draws on the CPU instead.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use gpui::{App, Global, RenderImage, Window, WindowId};

/// Rows of a read-back buffer start on multiples of this many bytes.
const ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

pub(crate) struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Whether the adapter is a software rasteriser.
    pub software: bool,
    /// The most samples a pixel the frame format takes, up to four.
    pub samples: u32,
}

/// The shared device, or `None` without one.
pub(crate) fn gpu() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| pollster::block_on(Gpu::new())).as_ref()
}

impl Gpu {
    async fn new() -> Option<Self> {
        if std::env::var("ATELIER_GPU").is_ok_and(|value| value == "off") {
            return None;
        }
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
        let software = adapter.get_info().device_type == wgpu::DeviceType::Cpu;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("atelier"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .ok()?;
        let supports = |format| {
            adapter
                .get_texture_format_features(format)
                .flags
                .sample_count_supported(4)
        };
        let samples = if supports(FORMAT) && supports(DEPTH_FORMAT) {
            4
        } else {
            1
        };
        Some(Self {
            device,
            queue,
            software,
            samples,
        })
    }

    /// Whether a `width` × `height` frame fits in a texture.
    pub fn fits(&self, width: u32, height: u32) -> bool {
        let largest = self.device.limits().max_texture_dimension_2d;
        width <= largest && height <= largest
    }

    /// The pixels of `texture`, a `FORMAT` texture, with the commands in `encoder` run first.
    pub fn read_back(
        &self,
        mut encoder: wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        readback: &wgpu::Buffer,
    ) -> Option<Vec<u8>> {
        let (width, height) = (texture.width(), texture.height());
        let row = padded_row(width);
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            extent(width, height),
        );
        self.queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
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
        readback.unmap();
        Some(pixels)
    }

    /// A buffer to read a `width` × `height` frame back through.
    pub fn readback(&self, width: u32, height: u32) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("atelier readback"),
            size: (padded_row(width) * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

/// The format frames are shaded in: what gpui's images hold.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The depth format of 3-D frames.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn padded_row(width: u32) -> u32 {
    (width * 4).div_ceil(ROW_ALIGNMENT) * ROW_ALIGNMENT
}

/// Frames handed to gpui, per window, so each can be taken out of gpui's sprite atlas once no
/// drawn scene shows it. gpui presents its latest scene again without drawing, so a frame is
/// kept until a draw after the last one that painted it.
#[derive(Default)]
pub(crate) struct Frames {
    windows: HashMap<WindowId, WindowFrames>,
}

#[derive(Default)]
struct WindowFrames {
    /// Bumped before each frame gpui draws.
    draw: u64,
    /// Whether a bump is already asked for.
    pending: bool,
    frames: HashMap<u64, (Arc<RenderImage>, u64)>,
}

impl Global for Frames {}

impl Frames {
    /// The frame for `key` in this window, rendered with `render` unless the last draw or this
    /// one already has it. Frames neither of them painted are dropped.
    pub fn get(
        key: u64,
        window: &mut Window,
        cx: &mut App,
        render: impl FnOnce() -> Option<Arc<RenderImage>>,
    ) -> Option<Arc<RenderImage>> {
        let id = window.window_handle().window_id();
        let frames = cx.default_global::<Frames>();
        let state = frames.windows.entry(id).or_default();
        let draw = state.draw;
        if !state.pending {
            state.pending = true;
            window.on_next_frame(move |_, cx| {
                if let Some(state) = cx.default_global::<Frames>().windows.get_mut(&id) {
                    state.draw += 1;
                    state.pending = false;
                }
            });
        }
        let stale: Vec<u64> = state
            .frames
            .iter()
            .filter(|(_, (_, last))| last + 1 < draw)
            .map(|(key, _)| *key)
            .collect();
        let stale: Vec<_> = stale
            .into_iter()
            .filter_map(|key| state.frames.remove(&key))
            .collect();
        let image = match state.frames.get_mut(&key) {
            Some((image, last)) => {
                *last = draw;
                Some(image.clone())
            }
            None => None,
        };
        for (image, _) in stale {
            window.drop_image(image).ok();
        }
        if image.is_some() {
            return image;
        }
        let image = render()?;
        let frames = cx.default_global::<Frames>();
        frames
            .windows
            .entry(id)
            .or_default()
            .frames
            .insert(key, (image.clone(), draw));
        Some(image)
    }
}
