use log::debug;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, DeviceDescriptor, Face, FragmentState, FrontFace,
    IndexFormat, Instance, InstanceDescriptor, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, ShaderStages, Surface,
    SurfaceConfiguration, SurfaceError, SurfaceTexture, TextureUsages, TextureView,
    VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode,
};
use winit::window::Window;

unsafe fn as_u8_slice<T>(x: &[T]) -> &[u8] {
    std::slice::from_raw_parts(x.as_ptr() as *const u8, std::mem::size_of_val(x))
}

unsafe fn as_raw_bytes<T>(x: &T) -> &[u8] {
    std::slice::from_raw_parts(x as *const T as *const u8, std::mem::size_of::<T>())
}

/// Rendering context
pub struct Context {
    /// Handle to the device we'll use to draw
    device: Device,

    /// Command queue for the selected device.
    commands: Queue,

    /// Window surface, render target
    surface: Surface,

    /// Configuration data for the surface.
    /// This is reused during `resize` operations.
    config: SurfaceConfiguration,
}

impl Context {
    pub async fn with_window(window: &Window) -> Self {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, commands) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    features: Default::default(),
                    limits: Default::default(),
                },
                None,
            )
            .await
            .unwrap();

        let config = {
            let caps = surface.get_capabilities(&adapter);
            // pick an srgb format if available
            let format = caps
                .formats
                .iter()
                .filter(|&f| f.is_srgb())
                .copied()
                .next()
                .unwrap_or(caps.formats[0]);
            let size = window.inner_size();
            SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.min(2),
                height: size.height.min(2),
                present_mode: caps.present_modes[0],
                alpha_mode: caps.alpha_modes[0],
                view_formats: Default::default(),
            }
        };
        surface.configure(&device, &config);

        Self {
            device,
            commands,
            surface,
            config,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn reset(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    pub fn get_next_frame(&self) -> Result<SurfaceTexture, SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn make_rect_painter(&self) -> Painter {
        Painter::new(self)
    }
}

#[repr(C)]
pub struct Vertex {
    pub xyz: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRS: [VertexAttribute; 2] = vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2
    ];

    fn layout<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as _,
            step_mode: VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct PainterSettings {
    pub edge: [f32; 4],
    pub fill: [f32; 4],
    pub line_width: f32,
}

impl PainterSettings {
    fn descriptor<'a>() -> BufferDescriptor<'a> {
        BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Self>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }
    }
}

impl Default for PainterSettings {
    fn default() -> Self {
        Self {
            edge: [0.0, 0.0, 0.0, 1.0],
            fill: [1.0, 1.0, 1.0, 1.0],
            line_width: 2.0,
        }
    }
}

pub struct Painter {
    pipeline: RenderPipeline,
    bind_group: BindGroup,
    uniforms: Buffer,
    vertices: Buffer,
    vertex_count: usize,
    indexes: Buffer,
    index_count: usize,
}

impl Painter {
    fn new(rc: &Context) -> Self {
        // Memory layout for the painter
        let layout = rc
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("my painter bind group layout"),
                entries: &[
                    // Color
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let uniforms = rc.device.create_buffer(&PainterSettings::descriptor());

        let bind_group = rc.device.create_bind_group(&BindGroupDescriptor {
            label: Some("My painter bind group"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let module = &rc.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("My Painter shader module"),
            source: ShaderSource::Wgsl(include_str!("painter.wgsl").into()),
        });

        let pipeline = rc.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("My Painter Render Pipeline"),
            layout: Some(
                &rc.device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("My Painter Render Pipeline Layout"),
                    bind_group_layouts: &[&layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: VertexState {
                module,
                entry_point: "vs",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(FragmentState {
                module,
                entry_point: "fs",
                targets: &[Some(ColorTargetState {
                    format: rc.config.format,
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Geometry buffers
        let vertices = rc.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Painter vertex buffer"),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            contents: &[0; 6000], // FIXME: reallocation?
        });

        let indexes = rc.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Painter index buffer"),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            contents: &[0; 6000], // FIXME: reallocation?
        });

        Self {
            pipeline,
            bind_group,
            uniforms,
            vertices,
            vertex_count: 0,
            indexes,
            index_count: 0,
        }
    }

    pub fn set_geometry(&mut self, rc: &Context, vertices: &[Vertex], indexes: &[u32]) {
        self.vertex_count = vertices.len();
        self.index_count = indexes.len();
        rc.commands
            .write_buffer(&self.vertices, 0, unsafe { as_u8_slice(vertices) });
        debug!("Writing index buffer. {:?}", unsafe {
            as_u8_slice(indexes)
        });
        rc.commands
            .write_buffer(&self.indexes, 0, unsafe { as_u8_slice(indexes) });
        // self.rc.commands.submit(None);
    }

    pub fn set_uniforms(&self, rc: &Context, settings: &PainterSettings) {
        rc.commands
            .write_buffer(&self.uniforms, 0, unsafe { as_raw_bytes(settings) });
        // self.rc.commands.submit(None);
    }

    pub fn draw(
        &self,
        rc: &Context,
        view: &TextureView,
        clear_color: Color,
    ) -> Result<(), SurfaceError> {
        let mut commands = rc
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let mut pass = commands.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear_color),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            debug!(
                "vertex count {} size {} {:?}",
                self.vertex_count,
                self.vertices.size(),
                self.vertices
            );
            pass.set_vertex_buffer(
                0,
                self.vertices
                    .slice(..(std::mem::size_of::<Vertex>() * self.vertex_count) as u64),
            );
            debug!(
                "index count {} size {} {:?}",
                self.index_count,
                self.indexes.size(),
                self.indexes
            );
            pass.set_index_buffer(
                self.indexes
                    .slice(..(std::mem::size_of::<u32>() * self.index_count) as u64),
                IndexFormat::Uint32,
            );
            pass.draw_indexed(0..self.index_count as u32, 0, 0..1);
        }
        rc.commands.submit(std::iter::once(commands.finish()));
        Ok(())
    }
}
