use nalgebra::Vector3;

use super::{DEPTH, HEIGHT, WIDTH};


// 世界坐标系使用 笛卡尔坐标系(右手坐标系)


/// 将obj的vertices坐标系 转为 笛卡尔坐标系
pub fn obj_to_cartesian(v: wavefront_obj::obj::Vertex) -> Vector3<f64> {
    
    let x = (v.x + 1.) * (WIDTH as f64 - 0.) / 2. + 0.;
    let y = (v.y + 1.) * (HEIGHT as f64 - 0.) / 2. + 0.;
    let z = (v.z + 1.) * (DEPTH as f64 - 0.) / 2. + 0.;
    let x = x - (WIDTH as f64 / 2.);
    let y = y - (HEIGHT as f64 / 2.);
    let z = z - (DEPTH as f64 / 2.);
    Vector3::new(x, y, z)
}

