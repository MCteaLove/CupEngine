use std::mem::size_of;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::Camera;
use crate::mesh::{MeshData, Vertex};

const MAX_MESH_INSTANCES: usize = 100_000;
const MAX_BILLBOARDS: usize = 100_000;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const VERTEX_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
const MESH_INSTANCE_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4
];
const BILLBOARD_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshId(usize);

#[derive(Clone, Copy, Debug)]
pub struct DirectionalLight {
    /// Направление, в котором светит свет.
    pub direction: Vec3,
    pub color: Vec3,
    pub ambient: Vec3,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -1.0, -0.3).normalize(),
            color: Vec3::new(1.0, 0.96, 0.9),
            ambient: Vec3::new(0.12, 0.13, 0.18),
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub mesh_instances: u32,
    pub billboards: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    cam_pos: [f32; 4],
    light_dir: [f32; 4],
    light_color: [f32; 4],
    ambient: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshInstance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BillboardInstance {
    pos_r: [f32; 4],
    color: [f32; 4],
}

struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,

    mesh_pipeline: wgpu::RenderPipeline,
    billboard_pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh_instance_buf: wgpu::Buffer,
    billboard_buf: wgpu::Buffer,

    meshes: Vec<GpuMesh>,
    mesh_queue: Vec<Vec<MeshInstance>>,
    billboards: Vec<BillboardInstance>,
    scratch: Vec<MeshInstance>,
    ranges: Vec<(usize, u32, u32)>,

    camera: Camera,
    light: DirectionalLight,
    pub clear_color: [f64; 3],
    pub stats: RenderStats,
}

fn create_depth_view(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

impl Renderer {
    pub(crate) async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).expect("surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth_view = create_depth_view(&device, config.width, config.height);

        // ---- uniform + bind group ----
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        // ---- mesh pipeline ----
        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh.wgsl").into()),
        });
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &mesh_shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &VERTEX_ATTRS,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: size_of::<MeshInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &MESH_INSTANCE_ATTRS,
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // ---- billboard pipeline ----
        let bb_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("billboard_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/billboard.wgsl").into()),
        });
        let billboard_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("billboard_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &bb_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<BillboardInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &BILLBOARD_ATTRS,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bb_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false, // тест глубины есть, запись нет
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let mesh_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh_instances"),
            size: (MAX_MESH_INSTANCES * size_of::<MeshInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let billboard_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("billboards"),
            size: (MAX_BILLBOARDS * size_of::<BillboardInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            mesh_pipeline,
            billboard_pipeline,
            globals_buf,
            bind_group,
            mesh_instance_buf,
            billboard_buf,
            meshes: Vec::new(),
            mesh_queue: Vec::new(),
            billboards: Vec::new(),
            scratch: Vec::new(),
            ranges: Vec::new(),
            camera: Camera::default(),
            light: DirectionalLight::default(),
            clear_color: [0.02, 0.025, 0.05],
            stats: RenderStats::default(),
        }
    }

    pub(crate) fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, w, h);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    // ------------------------------------------------------------------
    // Публичное API для игры
    // ------------------------------------------------------------------

    /// Загружает меш в GPU и возвращает handle.
    pub fn create_mesh(&mut self, data: &MeshData) -> MeshId {
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_vbuf"),
            contents: bytemuck::cast_slice(&data.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_ibuf"),
            contents: bytemuck::cast_slice(&data.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.push(GpuMesh { vbuf, ibuf, index_count: data.indices.len() as u32 });
        self.mesh_queue.push(Vec::new());
        MeshId(self.meshes.len() - 1)
    }

    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }

    pub fn set_light(&mut self, light: DirectionalLight) {
        self.light = light;
    }

    /// Поставить меш в очередь на отрисовку в этом кадре.
    pub fn draw_mesh(&mut self, mesh: MeshId, transform: Mat4, color: Vec4) {
        self.mesh_queue[mesh.0].push(MeshInstance {
            model: transform.to_cols_array_2d(),
            color: color.to_array(),
        });
    }

    /// Круглый светящийся биллборд (пули, частицы).
    pub fn draw_billboard(&mut self, pos: Vec3, radius: f32, color: Vec4) {
        self.billboards.push(BillboardInstance {
            pos_r: [pos.x, pos.y, pos.z, radius],
            color: color.to_array(),
        });
    }

    // ------------------------------------------------------------------
    // Кадр
    // ------------------------------------------------------------------

    pub(crate) fn render_frame(&mut self) {
        // 1. Собираем инстансы мешей в один непрерывный буфер, сгруппированный по мешу.
        self.scratch.clear();
        self.ranges.clear();
        for (idx, list) in self.mesh_queue.iter_mut().enumerate() {
            if list.is_empty() {
                continue;
            }
            let room = MAX_MESH_INSTANCES - self.scratch.len();
            let take = list.len().min(room);
            if take > 0 {
                let start = self.scratch.len() as u32;
                self.scratch.extend_from_slice(&list[..take]);
                self.ranges.push((idx, start, take as u32));
            }
            list.clear();
        }
        self.billboards.truncate(MAX_BILLBOARDS);

        // 2. Глобальные данные: камера и свет.
        let view = self.camera.view();
        let proj = self.camera.proj(self.aspect());
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let eye = self.camera.eye;
        let l = self.light;
        let globals = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            cam_right: [right.x, right.y, right.z, 0.0],
            cam_up: [up.x, up.y, up.z, 0.0],
            cam_pos: [eye.x, eye.y, eye.z, 1.0],
            light_dir: [l.direction.x, l.direction.y, l.direction.z, 0.0],
            light_color: [l.color.x, l.color.y, l.color.z, 1.0],
            ambient: [l.ambient.x, l.ambient.y, l.ambient.z, 1.0],
        };

        // 3. Получаем кадр.
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.billboards.clear();
                return;
            }
            Err(e) => {
                log::error!("surface error: {e:?}");
                self.billboards.clear();
                return;
            }
        };
        let view_tex = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 4. Загружаем данные.
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        if !self.scratch.is_empty() {
            self.queue.write_buffer(&self.mesh_instance_buf, 0, bytemuck::cast_slice(&self.scratch));
        }
        if !self.billboards.is_empty() {
            self.queue.write_buffer(&self.billboard_buf, 0, bytemuck::cast_slice(&self.billboards));
        }

        // 5. Рисуем.
        let mut draw_calls = 0u32;
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let c = self.clear_color;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_tex,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c[0], g: c[1], b: c[2], a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // меши
            pass.set_pipeline(&self.mesh_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            let stride = size_of::<MeshInstance>() as u64;
            for &(idx, start, count) in &self.ranges {
                let mesh = &self.meshes[idx];
                pass.set_vertex_buffer(0, mesh.vbuf.slice(..));
                pass.set_vertex_buffer(
                    1,
                    self.mesh_instance_buf.slice(start as u64 * stride..(start + count) as u64 * stride),
                );
                pass.set_index_buffer(mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..count);
                draw_calls += 1;
            }

            // биллборды (после мешей, чтобы прятались за геометрией)
            if !self.billboards.is_empty() {
                let n = self.billboards.len() as u32;
                pass.set_pipeline(&self.billboard_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.billboard_buf.slice(..));
                pass.draw(0..6, 0..n);
                draw_calls += 1;
            }
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();

        self.stats = RenderStats {
            draw_calls,
            mesh_instances: self.scratch.len() as u32,
            billboards: self.billboards.len() as u32,
        };
        self.billboards.clear();
    }
}
