use nalgebra::Vector3;




/// 获取三角形的法向量
/// 未标准化
pub fn normal_v(
    a: Vector3<f64>,
    b: Vector3<f64>,
    c: Vector3<f64>,
)->Vector3<f64>{
    nalgebra::base::Matrix::cross(&(a - c), &(b - c))
}
