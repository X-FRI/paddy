use minifb::{Key, KeyRepeat};
use nalgebra::{Matrix4, RowVector4, Vector2, Vector3, Vector4};
use num::integer::sqrt;

use crate::render::{
    buffer_fill_black, color::Color, create_buffer, create_window, draw, update_with_buffer,
};

use super::{DEPTH, HEIGHT, WIDTH};

/// 世界坐标系 使用 笛卡尔坐标系(右手坐标系)
/// 相机坐标系是 笛卡尔坐标系(右手坐标系)
/// 默认投射到 z=-1 上
/// ! 禁止使用这个结构,它只是一个参考
struct Camera {
    pub eye: Vector3<f64>,
    pub eye_target: Vector3<f64>,
    pub up: Vector3<f64>,
    /// 模型矩阵 (用于将 世界坐标系的点 转为 相机坐标系的点)
    pub model_view: Matrix4<f64>,
    /// 最近渲染距离
    pub z_near: f64,
    /// 最远渲染距离
    /// 0 > z_near > z_far
    pub z_far: f64,
    /// z=-1的视口 左点 < 0, 为了简单点,它总是y轴对称, 所以 right右点=-left
    pub left: f64,
    /// z=-1的视口 底点 < 0 , x轴对称 top=-bottom
    pub bottom: f64,
    /// 透视投影矩阵
    pub projection: Matrix4<f64>,
    /// 视口矩阵
    pub viewport: Matrix4<f64>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Default::default(),
            eye_target: Default::default(),
            up: Default::default(),
            left: -1.,
            bottom: -1.,
            z_near: -1.,
            z_far: -1000.,
            model_view: Default::default(),
            projection: Default::default(),
            // 因为被 normalize_m 压缩至 [-1,1]*[-1,1] 所以直接乘一半即可
            viewport: Matrix4::from_rows(&[
                RowVector4::new(WIDTH as f64 / 2., 0., 0., 0.),
                RowVector4::new(0., HEIGHT as f64 / 2., 0., 0.),
                RowVector4::new(0., 0., DEPTH as f64 / 2., DEPTH as f64 / 2.),
                RowVector4::new(0., 0., 0., 1.),
            ]),
        }
    }
}

/// 返回的矩阵可将 世界坐标上的点 转为 摄像机坐标上的点
/// eye : 眼睛坐标(世界坐标上的 摄像机坐标的原点)
/// center : 视野中心坐标(看向的方向坐标)
/// up : 头顶向量
/// @return : 模型矩阵
pub fn lookat(eye: Vector3<f64>, center: Vector3<f64>, up: Vector3<f64>) -> Matrix4<f64> {
    let z = (eye - center).normalize();
    let x = up.cross(&z).normalize();
    let y = z.cross(&x).normalize();
    let mut minv = Matrix4::<f64>::identity();
    let mut tr = Matrix4::<f64>::identity();
    for i in 0..3usize {
        minv[(0, i)] = x[i];
        minv[(1, i)] = y[i];
        minv[(2, i)] = z[i];
        tr[(i, 3)] = -eye[i];
    }
    let model_view = minv * tr;
    model_view
}

/// viewport * normalize_m * model_view * v
/// n.x 和 n.y 并非在中心, 而是左下角
/// 用于将 规范化后的点 转换到屏幕空间中 屏幕
/// @return 视口矩阵
pub fn viewport(n: Vector2<f64>, w: f64, h: f64) -> Matrix4<f64> {
    // let depth = 255.;
    // 将 x轴上放大 w/2 倍 在平移 x轴  x+w/2
    // 其他类似
    Matrix4::from_rows(&[
        RowVector4::new(w / 2., 0., 0., n.x + w / 2.),
        RowVector4::new(0., h / 2., 0., n.y + h / 2.),
        RowVector4::new(0., 0., DEPTH as f64 / 2., DEPTH as f64 / 2.),
        RowVector4::new(0., 0., 0., 1.),
    ])
}

/// 透视投影规范化
/// 0 > z_near > z_far
/// 0 > left, 0 > bottom
/// 对称性 left = -right , bottom = -top
/// 规范化到 [-1,1]*[-1,1]*[-1,1]
/// z=z_far平面映射到z=1上
/// z=z_near平面映射到z=-1上 , 所以z越小离摄像头越近
/// 规范后任是右手坐标系
/// @return 透视投影规范化矩阵
pub fn perspective_normalized(z_near: f64, z_far: f64, left: f64, bottom: f64) -> Matrix4<f64> {
    Matrix4::from_rows(&[
        RowVector4::new(z_near / -left, 0., 0., 0.),
        RowVector4::new(0., z_near / -bottom, 0., 0.),
        RowVector4::new(
            0.,
            0.,
            (z_near + z_far) / (z_far - z_near),
            (-2. * z_near * z_far) / (z_far - z_near),
        ),
        RowVector4::new(0., 0., 1., 0.),
    ])
}

/// 透视投影矩阵
/// 返回的矩阵将 投射到 z=-d(相机坐标系) 的平面上
/// 原点              变换后             进行投影
/// [x,y,z,1] ==> [x,y,z,-z/d] ==> [x,y,z,-z/d] * -d/z = [x*-d/z,y*-d/z,d(实际上只用关心xy即可,z还是用之前的),1]
pub fn projection_perspective(d: f64) -> Matrix4<f64> {
    Matrix4::from_rows(&[
        RowVector4::new(1., 0., 0., 0.),
        RowVector4::new(0., 1., 0., 0.),
        RowVector4::new(0., 0., 1., 0.),
        RowVector4::new(0., 0., -1. / d, 0.),
    ])
}

// 透视投影 转 正交投影 的矩阵
// n = near , f = far
// | n 0 0   0
// | 0 n 0   0
// | 0 0 n+f -fn
// | 0 0 1   0

// 观察空间(view space) 转换到 NDC空间 ,这样可以使得裁剪更加高效(有统一的前置标准)
// 我们的标准是 NDC空间为 [-1,1]*[-1,1]*[-1,1]

// 水平视场角的计算公式为 : a = 2 arctan(w/(2d))


mod tests {
    use num::Float;

    use super::*;

    #[test]
    fn test(){
        let (z_near, z_far, left, bottom) = (-10.,-20.,-1.,-1.);
        let m =     Matrix4::from_rows(&[
            RowVector4::new(z_near / -left, 0., 0., 0.),
            RowVector4::new(0., z_near / -bottom, 0., 0.),
            RowVector4::new(
                0.,
                0.,
                (z_near + z_far) / (z_far - z_near),
                (-2. * z_near * z_far) / (z_far - z_near),
            ),
            RowVector4::new(0., 0., 1., 0.),
        ]);
        let n_l_b = Vector4::new(left,bottom,z_near,1.);
        let mut f_l_b = n_l_b*2.;
        f_l_b.w = 1.;
        println!("{}",m*n_l_b);
        println!("{}",m*f_l_b);
        let n_o = Vector4::new(0.,0.,-10.,1.);
        println!("{}",m*n_o);
        let f_o = Vector4::new(0.,0.,-20.,1.);
        println!("{}",m*f_o);
        // 似乎是先做 位移 在做缩放 导致原点并非在原中心点
        let o = Vector4::new(0.,0.,-1000./75.,1.);
        println!("{}",m*o);

        println!("{}",m.try_inverse().unwrap()*Vector4::new(0.,0.,0.,1.));

    }

    #[test]
    fn test_xyz() {
        let mut window = create_window();
        let mut buffer = create_buffer();
    
        let mut eye = Vector3::new(110., 10., 0.);
        let center = Vector3::new(0., 0., 0.);
        let up = Vector3::new(0., 1., 0.);
    
        let mut model_view = lookat(eye, center, up);
        let normalize_m = perspective_normalized(-1., -200., -1., -1.);
        println!("{normalize_m:?}");
        let viewport = viewport([-500., -500.].into(), WIDTH as f64, HEIGHT as f64);
    
        let z = Vector4::new(0., 0., 100., 1.);
        let y = Vector4::new(0., 100., 0., 1.);
        let x = Vector4::new(100., 0., 0., 1.);
        let o = Vector4::new(0., 0., 0., 1.);
        // let z = model_view * z;
        // let y = model_view * y;
        // let x = model_view * x;
        // let o = model_view * o;
        // print!("x:{x}");
        // print!("{y}");
        // println!("{z}");
        // let z = normalize_m * z;
        // let y = normalize_m * y;
        // let x = normalize_m * x;
        // let o = normalize_m * o;
        // print!("x:{x}");
        // print!("{y}");
        // println!("{z}");
        // let z = viewport * z;
        // let y = viewport * y;
        // let x = viewport * x;
        // let o = viewport * o;
        // print!("x:{x}");
        // print!("{y}");
        // println!("{z}");
    
        let mut a = 0.;
        let mut r = (eye.x * eye.x + eye.y * eye.y + eye.z * eye.z).sqrt();
    
        while window.is_open() {
            buffer_fill_black(&mut buffer);
            model_view = lookat(eye, center, up);
    
            let z = viewport * normalize_m * model_view * z;
            let y = viewport * normalize_m * model_view * y;
            let x = viewport * normalize_m * model_view * x;
            let o = viewport * normalize_m * model_view * o;
    
            let n = |v: Vector4<f64>| -> Vector2<f64> { [v.x / v.w, v.y / v.w].into() };
    
            draw::line(&mut buffer, n(o), n(x), Color::Red);
            draw::line(&mut buffer, n(o), n(y), Color::Blue);
            draw::line(&mut buffer, n(o), n(z), Color::Yellow);
    
            window
                .get_keys_pressed(KeyRepeat::Yes)
                .iter()
                .for_each(|key| match key {
                    Key::A => {
                        a = a + 0.02;
                        eye.z = r * num::Float::sin(a);
                        eye.x = r * num::Float::cos(a);
                    }
                    Key::D => {
                        a = a - 0.02;
                        eye.z = r * num::Float::sin(a);
                        eye.x = r * num::Float::cos(a);
                    }
                    Key::W => {}
                    Key::S => {}
                    Key::C => {}
                    _ => {}
                });
            update_with_buffer(&mut window, &buffer);
        }
    }
    
}

