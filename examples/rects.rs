use std::f32::consts::PI;

use dotenv::dotenv;
use log::{debug, error, info};
use oreb::{Context, Painter, PainterSettings, Vertex};
use wgpu::{Color, TextureView, TextureViewDescriptor};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::macos::WindowBuilderExtMacOS,
    window::WindowBuilder,
};

struct Rect {
    center: [f32; 2],
    size: [f32; 2],
    orientation_radians: f32,
}

// x0,x1,y0,y1 are the bounds within which the rects should be generated.
// They should be in clip space.
fn make_rects(time_seconds: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<Rect> {
    let steps = 100;
    let dx = (x1 - x0) / (steps + 1) as f32;
    let dy = y1 - y0;
    let sz = dx.max(0.1);
    (0..steps)
        .map(|i| {
            let is_odd = (i & 1) == 1;
            let i = i as f32;
            let ph = 2.0 * PI * i / (steps + 1) as f32;
            let cx = x0 + dx * (i + 0.5);
            let cy = y0 + 0.5 * dy * (1.0 + (ph + 2.0 * PI * time_seconds / 7.0).cos());
            let w = 3.0 * sz; // + 1.0 * sz * (2.0 * PI * time_seconds / 0.5).cos();
            let h = 3.0 * sz; // + 1.0 * sz * (2.0 * PI * (0.3 + time_seconds / 3.0)).cos();
            let th = (2.0 * PI * time_seconds / 7.0) * if is_odd { 1.0 } else { -1.0 };
            Rect {
                center: [cx, cy],
                size: [w, h],
                orientation_radians: th,
            }
        })
        .collect()
}

fn encode_geometry(rects: &[Rect]) -> (Vec<Vertex>, Vec<u32>) {
    fn mk_vertices(rect: &Rect) -> [Vertex; 3] {
        let [cx, cy] = rect.center;
        let [half_w, half_h] = rect.size.map(|e| 0.5 * e);
        let side = half_h + half_w;
        let (s, c) = rect.orientation_radians.sin_cos();

        // create an isosceles right triangle within which the rect will be painted
        // center is at uv: [0,0]
        // rect's [w,h] in uv coords is [1,1]
        [
            // top-left
            Vertex {
                xyz: [-half_w, -half_h, 0.0],
                uv: [-0.5, -0.5],
            },
            // bottom-right
            Vertex {
                xyz: [2.0 * half_h - half_w, -half_h, 0.0],
                uv: [-0.5 + side / half_h, -0.5],
            },
            // bottom-left
            Vertex {
                xyz: [-half_w, 2.0 * half_w - half_h, 0.0],
                uv: [-0.5, -0.5 + side / half_w],
            },
        ]
        .map(|mut v| {
            // rotate about (0,0) by theta
            // then translate
            v.xyz[0] += half_w * 0.5;
            v.xyz[1] += half_h * 0.5;
            let x = v.xyz[0] * c - v.xyz[1] * s;
            let y = v.xyz[0] * s + v.xyz[1] * c;
            v.xyz[0] = x + cx;
            v.xyz[1] = y + cy;
            v
        })
    }

    let verts = rects
        .into_iter()
        .map(|r| mk_vertices(r))
        .flatten()
        .collect();
    let idxs = (0..3 * rects.len() as u32).collect();
    (verts, idxs)
}

fn draw(
    context: &Context,
    target: &TextureView,
    painter: &mut Painter,
    clear_color: Color,
    time_seconds: f32,
) {
    // 1. Generate some random rectangles
    // 2. encode geometry
    let (vs, is) = encode_geometry(&make_rects(time_seconds, -0.9, 0.9, -0.9, 0.9));
    // 3. stage
    painter.set_geometry(context, &vs, &is);
    // 4. draw
    painter.draw(context, target, clear_color);
}

#[async_std::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    info!("Hello world");

    let events = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Oreb: Rectangles")
        .with_transparent(false)
        .with_titlebar_transparent(true)
        .with_resizable(true)
        .with_inner_size(LogicalSize {
            width: 501,
            height: 501,
        })
        .build(&events)
        .expect("Failed to build window");

    let mut rc = Context::with_window(&window).await;
    let mut painter = rc.make_rect_painter();

    {
        let size = window.inner_size();
        painter.set_uniforms(
            &rc,
            &PainterSettings {
                edge: [0.0, 0.0, 0.0, 1.0],
                fill: [0.2, 0.2, 0.2, 0.5],
                line_width: 8.0,
            },
        );
    }

    let clear_color = Color {
        r: 0.3,
        g: 0.2,
        b: 0.1,
        a: 1.0,
    };

    let clock = std::time::Instant::now();
    let main_window_id = window.id();
    events.run(move |event, _, control_flow| match event {
        Event::RedrawRequested(window_id) if window_id == main_window_id => {
            match rc.get_next_frame() {
                Ok(frame) => {
                    let view = frame.texture.create_view(&TextureViewDescriptor::default());
                    draw(
                        &rc,
                        &view,
                        &mut painter,
                        clear_color,
                        clock.elapsed().as_secs_f32(),
                    );
                    frame.present();
                }
                Err(wgpu::SurfaceError::Lost) => rc.reset(),
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    error!("The render context appears out of memory. Exiting.");
                    *control_flow = ControlFlow::Exit;
                }
                Err(_) => todo!(),
            }
        }

        Event::MainEventsCleared => {
            window.request_redraw();
        }

        Event::WindowEvent { window_id, event } if window_id == main_window_id => match event {
            WindowEvent::Resized(size) => {
                rc.resize(size.width, size.height);
                window.request_redraw();
            }

            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                rc.resize(new_inner_size.width, new_inner_size.height);
                window.request_redraw();
            }

            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        },
        _ => {}
    });
}
