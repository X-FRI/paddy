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

/// 重心坐标法绘制\
/// 支持z_buffer的三角形绘制\
/// z_buffer 越小越近\
/// #untested
pub fn triangle_z<F>(
    buffer: &mut WindowBuffer,
    z_buffer: &mut ZBuffer,
    a: Vector3<f64>,
    b: Vector3<f64>,
    c: Vector3<f64>,
    color_f: F,
) where F: Fn(Vector3<f64>) -> Color {
    fn barycentric(
        a: Vector3<f64>,
        b: Vector3<f64>,
        c: Vector3<f64>,
        p: Vector3<f64>,
    ) -> Vector3<f64> {
        let mut s = vec![Vector3::new(0., 0., 0.), Vector3::new(0., 0., 0.)];
        for i in (0usize..2).rev() {
            s[i][0] = c[i] - a[i];
            s[i][1] = b[i] - a[i];
            s[i][2] = a[i] - p[i];
        }
        let u = nalgebra::base::Matrix::cross(&s[0], &s[1]);
        if u.z.abs() > 1e-3 {
            [1. - (u.x + u.y)/u.z, u.y / u.z, u.x / u.z].into()
        } else {
            [-1., 1., 1.].into()
        }
    }
    let mut bboxmin = Vector2::new(f64::MAX, f64::MAX);
    let mut bboxmax = Vector2::new(f64::MIN, f64::MIN);
    let clamp = Vector2::new((WIDTH/2) as f64 - 2., (HEIGHT/2) as f64 - 2.);
    let pts = vec![a, b, c];
    for i in 0..3 {
        for j in 0..2 {
            bboxmin[j] = bboxmin[j].min(pts[i][j]);
            bboxmax[j] = clamp[j].min(bboxmax[j].max(pts[i][j]));
        }
    }
    let mut P = Vector3::new(bboxmin.x, bboxmin.y, 0.);
    while P.x <= bboxmax.x {
        while P.y <= bboxmax.y {
            let bc_screen = barycentric(a, b, c, P);
            if (bc_screen.x < 0. || bc_screen.y < 0. || bc_screen.z < 0.) {
                P.y = P.y + 1.;
                continue;
            }
            P.z = 0.;
            for i in 0..3 {
                P.z = P.z + (pts[i].z * bc_screen[i]);
            }
            match get_z_buffer(z_buffer, P.x.round() as i32, P.y.round() as i32) {
                Some(v) if v > P.z => {
                    set_z_buffer(z_buffer, P.x.round() as i32, P.y.round() as i32, P.z);
                    draw::point(buffer, [P.x, P.y].into(),color_f(P));
                },
                _ => {},
            }
            P.y = P.y + 1.;
        }
        P.y = bboxmin.y;
        P.x = P.x + 1.;
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
