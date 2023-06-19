use log::debug;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendState, Buffer, BufferBindingType, BufferDescriptor,
    BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Face,
    FragmentState, FrontFace, IndexFormat, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, SurfaceError, TextureView, VertexAttribute,
    VertexBufferLayout, VertexState, VertexStepMode,
};

use crate::Context;

unsafe fn as_u8_slice<T>(x: &[T]) -> &[u8] {
    std::slice::from_raw_parts(x.as_ptr() as *const u8, std::mem::size_of_val(x))
}

unsafe fn as_raw_bytes<T>(x: &T) -> &[u8] {
    std::slice::from_raw_parts(x as *const T as *const u8, std::mem::size_of::<T>())
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
    pub line_width_px: f32,
    pub corner_radius_px: f32,
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
            line_width_px: 2.0,
            corner_radius_px: 0.0,
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
    pub(crate) fn new(rc: &Context) -> Self {
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
