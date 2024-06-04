use color::Color;
use nalgebra::Vector2;


use super::*;

/// 使用原坐标进行绘制点
fn point_(buffer: &mut WindowBuffer, dot: Vector2<f64>, color: Color) {
    if dot.x < 0. || dot.y < 0. || dot.x > 999. || dot.y > 999. {
        return; // 坐标系溢出,不进行绘制
    }
    buffer[dot.y.round() as usize * WIDTH + dot.x.round() as usize] = color.into();
}
/// 使用笛卡尔坐标系进行绘制点
pub fn point(buffer: &mut WindowBuffer, dot: Vector2<f64>, color: Color) {
    let x = dot.x + (WIDTH as f64 / 2.);
    let y = (-dot.y) + (HEIGHT as f64 / 2.);
    point_(buffer, [x, y].into(), color);
}

pub fn line(buffer: &mut WindowBuffer, mut a: Vector2<f64>, mut b: Vector2<f64>, color: Color) {
    let mut steep = false;
    if (a.x - b.x).abs() < (a.y - b.y).abs() {
        a = Vector2::new(a.y, a.x);
        b = Vector2::new(b.y, b.x);
        steep = true;
    }
    if a.x > b.x {
        swap(&mut a, &mut b);
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let derror = (dy / dx).abs();
    let mut error = 0.;
    let mut y = a.y;
    for x in (a.x.round() as i32)..=(b.x.round() as i32) {
        if steep {
            point(buffer, [y, x as f64].into(), color.clone());
        } else {
            point(buffer, [x as f64, y].into(), color.clone());
        }
        error = error + derror;
        if error > 0.5 {
            y = y + if b.y > a.y { 1. } else { -1. };
            error = error - 1.;
        }
    }
}

pub fn triangle_line(
    buffer: &mut WindowBuffer,
    a: Vector2<f64>,
    b: Vector2<f64>,
    c: Vector2<f64>,
    color: Color,
) {
    line(buffer, a, b, color);
    line(buffer, b, c, color);
    line(buffer, a, c, color);
}

/// 扫线法绘制
pub fn triangle(
    buffer: &mut WindowBuffer,
    mut a: Vector2<f64>,
    mut b: Vector2<f64>,
    mut c: Vector2<f64>,
    color: Color,
) {
    if a.y == b.y && a.y == c.y {
        return;
    }
    // 保证a点是y最高的点
    if a.y < b.y {
        swap(&mut a, &mut b);
    }
    if a.y < c.y {
        swap(&mut a, &mut c);
    }
    // 保证b点是第二高点
    if b.y < c.y {
        swap(&mut b, &mut c);
    }
    let total_height = (a.y - c.y).round() as u32;

    for i in 0..total_height {
        let second_half = i as f64 > b.y - c.y || b.y == c.y;
        let segment_height = if second_half { a.y - b.y } else { b.y - c.y };
        let alpha = i as f64 / total_height as f64;
        let beta = (i as f64 - (if second_half { b.y - c.y } else { 0. })) / segment_height;
        let mut A: Vector2<f64> = c + (a - c).map(|v| v * alpha);
        let mut B: Vector2<f64> = if second_half {
            b + (a - b).map(|v| v * beta)
        } else {
            c + (b - c).map(|v| v * beta)
        };
        if A.x > B.x {
            swap(&mut A, &mut B);
        }
        for j in A.x.round() as i32..=B.x.round() as i32 {
            point(buffer, [j as f64, c.y + i as f64].into(), color);
        }
    }
}



#[test]
fn test_triangle(){
    let mut window = create_window();
    let mut buffer = create_buffer();
    let x = Vector2::new(-100., 0.);
    let y = Vector2::new(0., -100.);
    let o = Vector2::new(0., 0.);

    triangle(&mut buffer, x, y, o, Color::Cyan);
    println!("{x},{y}");
    while window.is_open() {
        update_with_buffer(&mut window, &buffer);
    }
}
