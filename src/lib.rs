pub mod rect;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use wgpu::{
    Backends, Device, DeviceDescriptor, Instance, InstanceDescriptor, Queue, RequestAdapterOptions,
    Surface, SurfaceConfiguration, SurfaceError, SurfaceTexture, TextureUsages,
};

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
    pub async fn with_window<W>(window: &W, width: u32, height: u32) -> Self
    where
        W: HasRawWindowHandle + HasRawDisplayHandle,
    {
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
            SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.min(2),
                height: height.min(2),
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

    pub fn make_rect_painter(&self) -> rect::Painter {
        rect::Painter::new(self)
    }
}
