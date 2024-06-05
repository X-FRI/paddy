use nalgebra::Vector3;

use super::color::Color;

/// 点光源
struct PointLight {
    /// 光在世界坐标中的位置
    position: Vector3<f64>,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}

/// 平行光
struct DirectionalLight {
    /// 光的方向向量
    direction: Vector3<f64>,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}

/// 聚光源
struct SpotLight {
    /// 光在世界坐标中的位置
    position: Vector3<f64>,
    /// 光照目标 ,即 光照的方向向量
    target: Vector3<f64>,
    /// 聚光源发散角度
    angle:f64,
    /// 光源衰减参数 (随着距离的改变，光线会衰减，越来越弱)
    /// 衰减算法可能会采取: 
    /// r:是距离 , a:是一个较小的值(小到不影响效果即可,为了优化r接近0时导致无穷大的情况),根据实际情况取值(默认取 1)
    /// r_max: 是r的最大上限
    /// f_win(r) = (1-(r/r_max)^4)^{+2}  //b^{+2}表示若b表达式为负数,则当前表示式为0,若为正数才进行二次方运算
    /// 衰减算法为: f_win(r) * (decay^2/(r^2+a))
    decay:f64,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}
impl SpotLight {
    /// 距离光源衰减函数 (distance falloff function)
    /// 
    pub fn dist(){

    }
    
}

/// 环境光
struct AmbientLight {}
