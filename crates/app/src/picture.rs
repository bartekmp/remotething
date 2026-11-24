//! Draws the remote desktop through one GPU texture that is updated in place.
//!
//! Showing every frame as a new image makes the toolkit create and drop a
//! texture per frame, which blinks on some drivers.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use iced::wgpu;
use iced::widget::image::Handle;
use iced::widget::shader::{self, Viewport};
use iced::widget::{image, shader as shader_widget};
use iced::{ContentFit, Element, Length, Rectangle, Size, mouse};

/// One picture of the remote desktop: RGBA bytes, cheap to clone.
#[derive(Clone)]
pub(crate) struct Picture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    rgba: Arc<Vec<u8>>,
    /// Differs for every picture, so the texture is only rewritten when it changed.
    serial: u64,
}

impl fmt::Debug for Picture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Picture({}x{})", self.width, self.height)
    }
}

impl Picture {
    pub(crate) fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);
        Self {
            width,
            height,
            rgba: Arc::new(rgba),
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// A number that differs between pictures, for telling a changed one from an unchanged one.
    pub(crate) fn serial(&self) -> u64 {
        self.serial
    }

    /// The pixels, shared: cloning the handle is cheap.
    pub(crate) fn pixels(&self) -> Arc<Vec<u8>> {
        Arc::clone(&self.rgba)
    }

    /// One black pixel, shown until the first real picture arrives.
    pub(crate) fn black() -> Self {
        Self::new(1, 1, vec![0, 0, 0, 255])
    }
}

static GPU_DRAWING: OnceLock<bool> = OnceLock::new();

/// Looks for a graphics adapter. Without one the toolkit draws on the CPU,
/// and that renderer cannot run the texture shader below.
///
/// Call once at start-up, before the first picture is drawn.
pub(crate) fn detect_gpu() {
    let forced_cpu = std::env::var("ICED_BACKEND").is_ok_and(|backend| backend == "tiny-skia");
    let found = !forced_cpu && {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let options = wgpu::RequestAdapterOptions::default();
        iced::futures::executor::block_on(instance.request_adapter(&options)).is_ok()
    };
    let _ = GPU_DRAWING.set(found);
}

/// Whether the window is drawn on a GPU. True until start-up says otherwise.
pub(crate) fn gpu_drawing() -> bool {
    GPU_DRAWING.get().copied().unwrap_or(true)
}

/// The picture as a widget, scaled to fit and centred: through the texture on
/// a GPU, as a plain image on the CPU renderer.
pub(crate) fn view<'a, Message: 'a>(picture: &Picture) -> Element<'a, Message> {
    if gpu_drawing() {
        shader_widget(Screen(picture.clone()))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let handle = Handle::from_rgba(picture.width, picture.height, picture.rgba.to_vec());
        image(handle)
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// The widget's program: draws a picture scaled to fit its bounds.
struct Screen(pub(crate) Picture);

impl<Message> shader::Program<Message> for Screen {
    type State = ();
    type Primitive = Drawing;

    fn draw(&self, _state: &(), _cursor: mouse::Cursor, _bounds: Rectangle) -> Drawing {
        Drawing(self.0.clone())
    }
}

#[derive(Debug)]
pub(crate) struct Drawing(Picture);

/// How much of the widget's width and height the picture covers when it is
/// scaled to fit and centred, as fractions from 0 to 1.
pub(crate) fn covered(bounds: Size, picture: (u32, u32)) -> (f32, f32) {
    let (width, height) = (picture.0 as f32, picture.1 as f32);
    if width <= 0.0 || height <= 0.0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = (bounds.width / width).min(bounds.height / height);
    (width * scale / bounds.width, height * scale / bounds.height)
}

const SHADER: &str = "
struct Cover { size: vec2<f32> };
@group(0) @binding(0) var<uniform> cover: Cover;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> Vertex {
    // Four corners of a strip: (0,0) (1,0) (0,1) (1,1).
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    var out: Vertex;
    out.position = vec4<f32>((corner.x * 2.0 - 1.0) * cover.size.x,
                             (1.0 - corner.y * 2.0) * cover.size.y, 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fragment_main(in: Vertex) -> @location(0) vec4<f32> {
    return textureSample(picture, smooth_sampler, in.uv);
}
";

/// The texture, shared by every desktop on screen.
struct Slot {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

pub(crate) struct Gpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cover: wgpu::Buffer,
    /// Matches the window surface: sRGB textures are decoded when sampled.
    texture_format: wgpu::TextureFormat,
    slot: Option<Slot>,
    uploaded: u64,
}

impl shader::Pipeline for Gpu {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("remote desktop"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("remote desktop"),
            entries: &[
                entry(
                    0,
                    wgpu::ShaderStages::VERTEX,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("remote desktop"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("remote desktop"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let cover = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("remote desktop"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture_format = if format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        Self {
            pipeline,
            layout,
            sampler,
            cover,
            texture_format,
            slot: None,
            uploaded: 0,
        }
    }
}

fn entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    }
}

impl Gpu {
    /// Makes sure there is a texture of this size, replacing the old one when the size differs.
    fn ensure_slot(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let reusable = self.slot.as_ref().is_some_and(|slot| slot.size == size);
        if !reusable {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("remote desktop"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.texture_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("remote desktop"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.cover.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.slot = Some(Slot {
                texture,
                bind_group,
                size,
            });
            // A new texture is empty.
            self.uploaded = 0;
        }
    }
}

impl shader::Primitive for Drawing {
    type Pipeline = Gpu;

    fn prepare(
        &self,
        gpu: &mut Gpu,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        let picture = &self.0;
        gpu.ensure_slot(device, (picture.width, picture.height));
        if let Some(slot) = &gpu.slot
            && gpu.uploaded != picture.serial
        {
            queue.write_texture(
                slot.texture.as_image_copy(),
                &picture.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * picture.width),
                    rows_per_image: Some(picture.height),
                },
                wgpu::Extent3d {
                    width: picture.width,
                    height: picture.height,
                    depth_or_array_layers: 1,
                },
            );
            gpu.uploaded = picture.serial;
        }
        let (across, down) = covered(bounds.size(), (picture.width, picture.height));
        let size = [across, down, 0.0, 0.0];
        queue.write_buffer(&gpu.cover, 0, &floats_to_bytes(size));
    }

    fn draw(&self, gpu: &Gpu, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(slot) = &gpu.slot else {
            return true;
        };
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &slot.bind_group, &[]);
        pass.draw(0..4, 0..1);
        true
    }
}

fn floats_to_bytes(values: [f32; 4]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
        chunk.copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_of_the_same_shape_fills_the_widget() {
        let covered = covered(Size::new(640.0, 400.0), (1280, 800));

        assert_eq!(covered, (1.0, 1.0));
    }

    #[test]
    fn a_wide_widget_leaves_bars_at_the_sides() {
        let (across, down) = covered(Size::new(1600.0, 400.0), (1280, 800));

        assert_eq!(down, 1.0);
        assert!((across - 0.4).abs() < 1e-6);
    }

    #[test]
    fn a_tall_widget_leaves_bars_above_and_below() {
        let (across, down) = covered(Size::new(640.0, 800.0), (1280, 800));

        assert_eq!(across, 1.0);
        assert!((down - 0.5).abs() < 1e-6);
    }

    #[test]
    fn nothing_is_covered_when_there_is_no_room() {
        assert_eq!(covered(Size::new(0.0, 100.0), (1280, 800)), (0.0, 0.0));
    }

    #[test]
    fn every_picture_gets_its_own_serial() {
        assert_ne!(Picture::black().serial, Picture::black().serial);
    }
}
