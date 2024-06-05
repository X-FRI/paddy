use std::mem::swap;

use minifb::{ScaleMode, Window, WindowOptions};
use nalgebra::{Vector2, Vector3};


pub mod coordinate;
pub mod draw;
pub mod camera;
pub mod chaos;
pub mod color;
pub mod light;
pub mod shader;



pub const WIDTH: usize = 1000;
pub const HEIGHT: usize = 1000;
/// 给z轴用的
pub const DEPTH: usize = 1000;

pub type WindowBuffer = Vec<u32>;
pub type ZBuffer = Vec<f64>;

pub fn create_window() -> Window {
    // 创建一个窗口
    let mut window = Window::new(
        "Renderer 1",
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale_mode: ScaleMode::Center,
            // transparency: true,//似乎暂时只支持了 windows
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| {
        panic!("window create:{}", e);
    });
    window.set_target_fps(60);
    window
}
pub fn create_buffer() -> WindowBuffer {
    // 创建一个缓冲区，用来存储像素数据
    let buffer: WindowBuffer = vec![0; WIDTH * HEIGHT];
    buffer
}
pub fn create_z_buffer() -> ZBuffer {
    let z_buffer: ZBuffer = vec![0.; WIDTH * HEIGHT];
    z_buffer
}

pub fn update_with_buffer(window: &mut Window, buffer: &WindowBuffer) {
    window
        .update_with_buffer(buffer.as_slice(), WIDTH, HEIGHT)
        .unwrap();
}

pub fn buffer_fill_black(buffer: &mut WindowBuffer) {
    buffer.fill(0);
}







#[test]
fn test() {
    let mut window = create_window();
    let mut buffer = create_buffer();

    while window.is_open() {
        update_with_buffer(&mut window, &buffer);
    }
}
