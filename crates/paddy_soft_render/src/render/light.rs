use nalgebra::Vector3;

use super::color::Color;

/// 平行光
/// 往往作为世界光源(大面积光源)
struct DirectionalLight {
    /// 光的方向向量
    direction: Vector3<f64>,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}

/// 点光源
struct PointLight {
    /// 光在世界坐标中的位置
    position: Vector3<f64>,
    /// 光源衰减参数 (随着距离的改变，光线会衰减，越来越弱)
    decay: f64,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}
impl PointLight {}

/// 聚光源
/// s : 聚光方向向量
///  
struct SpotLight {
    /// 光在世界坐标中的位置
    position: Vector3<f64>,
    /// 光照目标 ,即 光照的方向向量
    target: Vector3<f64>,
    /// 聚光源发散角度
    angle: f64,
    /// 光源衰减参数 (随着距离的改变，光线会衰减，越来越弱)
    /// 衰减算法可能会采取:
    /// r:是距离 , a:是一个较小的值(小到不影响效果即可,为了优化r接近0时导致无穷大的情况),根据实际情况取值(默认取 1)
    /// r_max: 是r的最大上限
    /// f_win(r) = (1-(r/r_max)^4)^{+2}  //b^{+2}表示若b表达式为负数,则当前表示式为0,若为正数才进行二次方运算
    /// 衰减算法为: f_dist =  f_win(r) * (decay^2/(r^2+a))
    decay: f64,
    /// 光线色彩
    color: Color,
    /// 光照强度 [0,1]
    intensity: f64,
}
impl SpotLight {
    /// 距离光源衰减函数 (distance falloff function)\
    /// 平方反比衰减模式\
    /// #wait : 参数暂时未确定下来,
    pub fn dist_1() {}
    /// 距离光源衰减函数 (distance falloff function)\
    /// 指数衰减模式\
    /// #wait : 参数暂时未确定下来,
    pub fn dist_2() {}
}

/// 环境光
struct AmbientLight {}

mod tests {
    use std::{fs::File, io::Read};

    use minifb::{Key, KeyRepeat};
    use nalgebra::{Vector2, Vector4};
    use wavefront_obj::obj::Primitive;

    use crate::render::{
        buffer_fill_black,
        camera::{lookat, perspective_normalized, viewport},
        chaos::normal_v,
        create_buffer, create_window, create_z_buffer, draw, update_with_buffer, HEIGHT, WIDTH,
    };

    use super::*;

    /// 不知道为什么 计算压力 会让窗体崩溃,感觉也不大啊,算了,就只渲染一个方向吧,CPU不行的话,镜头不要动哦
    #[test]
    fn test_directional_light() {
        let mut window = create_window();
        let mut buffer = create_buffer();
        let mut z_buffer = create_z_buffer();

        let mut file = File::open("obj/test_render_light_cube.obj").unwrap();
        // let mut file = File::open("obj/african_head.obj").unwrap();
        let mut str = String::new();
        file.read_to_string(&mut str).unwrap();
        let obj_data = wavefront_obj::obj::parse(str).unwrap();
        // let obj = obj_data.objects.into_iter().next().unwrap();

        let mut eye = Vector3::new(100., 100., -100.);
        let center = Vector3::new(0., 0., 0.);
        let up = Vector3::new(0., 1., 0.);

        let mut model_view = lookat(eye, center, up);
        let perspective_m = perspective_normalized(-1., -200., -1., -1.);
        let viewport = viewport([-500., -500.].into(), WIDTH as f64, HEIGHT as f64);

        let dir_light = DirectionalLight {
            direction: Vector3::new(1., 3., 2.).normalize(),
            color: Color::White,
            intensity: 1.,
        };

        let z = Vector4::new(0., 0., 200., 1.);
        let y = Vector4::new(0., 200., 0., 1.);
        let x = Vector4::new(200., 0., 0., 1.);
        let o = Vector4::new(0., 0., 0., 1.);
        let z = viewport * perspective_m * model_view * z;
        let y = viewport * perspective_m * model_view * y;
        let x = viewport * perspective_m * model_view * x;
        let o = viewport * perspective_m * model_view * o;
        let n = |v: Vector4<f64>| -> Vector2<f64> { [v.x / v.w, v.y / v.w].into() };
        draw::line(&mut buffer, n(o), n(x), Color::Red);
        draw::line(&mut buffer, n(o), n(y), Color::Blue);
        draw::line(&mut buffer, n(o), n(z), Color::Yellow);

        let mut refresh = true;
        while window.is_open() {
            if refresh {
                buffer_fill_black(&mut buffer);
                for obj in &obj_data.objects {
                    for geometry in &obj.geometry {
                        for shape in &geometry.shapes {
                            match shape.primitive {
                                Primitive::Point(_) => {}
                                Primitive::Line(_, _) => {}
                                Primitive::Triangle((a, .., an), (b, .., bn), (c, .., cn)) => {
                                    let v0 = obj.vertices[a];
                                    let v1 = obj.vertices[b];
                                    let v2 = obj.vertices[c];
                                    // let n0 = obj.normals[an.unwrap()];
                                    // let n1 = obj.normals[bn.unwrap()];
                                    // let n2 = obj.normals[cn.unwrap()];
                                    let v0 = Vector3::new(v0.x, v0.y, v0.z);
                                    let v1 = Vector3::new(v1.x, v1.y, v1.z);
                                    let v2 = Vector3::new(v2.x, v2.y, v2.z);
                                    let n = normal_v(v0, v1, v2).normalize();
                                    let mut v0 = v0.to_homogeneous();
                                    v0.w = 1.0;
                                    let mut v1 = v1.to_homogeneous();
                                    v1.w = 1.0;
                                    let mut v2 = v2.to_homogeneous();
                                    v2.w = 1.0;
                                    let intensity =
                                        nalgebra::base::Matrix::dot(&n, &dir_light.direction);
                                    let v0 = viewport * perspective_m * model_view * v0;
                                    let v1 = viewport * perspective_m * model_view * v1;
                                    let v2 = viewport * perspective_m * model_view * v2;
                                    let v0 = Vector3::new(v0.x / v0.w, v0.y / v0.w, v0.z / v0.w);
                                    let v1 = Vector3::new(v1.x / v1.w, v1.y / v1.w, v1.z / v1.w);
                                    let v2 = Vector3::new(v2.x / v2.w, v2.y / v2.w, v2.z / v2.w);
                                    if intensity > 0. {
                                        draw::triangle_z(
                                            &mut buffer,
                                            &mut z_buffer,
                                            v0,
                                            v1,
                                            v2,
                                            |p| {
                                                dir_light
                                                    .color
                                                    .mul(intensity)
                                                    .mul(dir_light.intensity)
                                            },
                                        );
                                    } else {
                                        draw::triangle_z(
                                            &mut buffer,
                                            &mut z_buffer,
                                            v0,
                                            v1,
                                            v2,
                                            |p| dir_light.color.mul(0.1),
                                        );
                                    }
                                    // draw::triangle_line(
                                    //     &mut buffer,
                                    //     [v0.x, v0.y].into(),
                                    //     [v1.x, v1.y].into(),
                                    //     [v2.x, v2.y].into(),
                                    //     Color::Red,
                                    // );
                                }
                            }
                        }
                    }
                }
                refresh = false;
            }

            for key in window.get_keys_pressed(KeyRepeat::Yes).iter() {
                refresh = true;
                match key {
                    Key::A => {
                        eye.x = eye.x + 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::D => {
                        eye.x = eye.x - 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::W => {
                        eye.y = eye.y + 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::S => {
                        eye.y = eye.y - 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::E => {
                        eye.z = eye.z + 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::Q => {
                        eye.z = eye.z - 3.;
                        model_view = lookat(eye, center, up);
                    }
                    Key::C => {}
                    _ => {}
                }
            }
            update_with_buffer(&mut window, &buffer);
        }
    }

    /// 点光源 实现的并不完整 ,仅作为参考
    #[test]
    fn test_point_light() {
        let mut window = create_window();
        let mut buffer = create_buffer();
        let mut z_buffer = create_z_buffer();

        let mut file = File::open("obj/test_render_light_cube.obj").unwrap();
        // let mut file = File::open("obj/african_head.obj").unwrap();
        let mut str = String::new();
        file.read_to_string(&mut str).unwrap();
        let obj_data = wavefront_obj::obj::parse(str).unwrap();
        // let obj = obj_data.objects.into_iter().next().unwrap();

        let mut eye = Vector3::new(-100., 100., 100.);
        let center = Vector3::new(0., 0., 0.);
        let up = Vector3::new(0., 1., 0.);

        let mut model_view = lookat(eye, center, up);
        let perspective_m = perspective_normalized(-1., -200., -1., -1.);
        let viewport = viewport([-500., -500.].into(), WIDTH as f64, HEIGHT as f64);

        let mut point_light = PointLight {
            position: Vector3::new(-50., 50., -70.),
            decay: 20.,
            color: Color::White,
            intensity: 1.,
        };

        fn dist(decay: f64, r: f64, r_max: f64) -> f64 {
            let f_win = 1. - (r / r_max).powf(4.);
            let f_win = if f_win > 0. { f_win.powf(2.) } else { 0. };
            let a = decay.powf(2.) / (r.powf(2.) + 1.);
            return f_win * a;
        }

        let z = Vector4::new(0., 0., 500., 1.);
        let y = Vector4::new(0., 500., 0., 1.);
        let x = Vector4::new(500., 0., 0., 1.);
        let o = Vector4::new(0., 0., 0., 1.);
        let mut pl = point_light.position.to_homogeneous();
        pl.w = 1.;
        let z = viewport * perspective_m * model_view * z;
        let y = viewport * perspective_m * model_view * y;
        let x = viewport * perspective_m * model_view * x;
        let o = viewport * perspective_m * model_view * o;
        let pl = viewport * perspective_m * model_view * pl;
        point_light.position = Vector3::new(pl.x / pl.w, pl.y / pl.w, pl.z / pl.w);
        let n = |v: Vector4<f64>| -> Vector2<f64> { [v.x / v.w, v.y / v.w].into() };
        draw::line(&mut buffer, n(o), n(x), Color::Red);
        draw::line(&mut buffer, n(o), n(y), Color::Blue);
        draw::line(&mut buffer, n(o), n(z), Color::Yellow);
        draw::line(&mut buffer, n(o), n(pl), Color::Magenta);

        for obj in &obj_data.objects {
            for geometry in &obj.geometry {
                for shape in &geometry.shapes {
                    match shape.primitive {
                        Primitive::Point(_) => {}
                        Primitive::Line(_, _) => {}
                        Primitive::Triangle((a, .., an), (b, .., bn), (c, .., cn)) => {
                            let v0 = obj.vertices[a];
                            let v1 = obj.vertices[b];
                            let v2 = obj.vertices[c];
                            // let n0 = obj.normals[an.unwrap()];
                            // let n1 = obj.normals[bn.unwrap()];
                            // let n2 = obj.normals[cn.unwrap()];
                            let v0 = Vector3::new(v0.x, v0.y, v0.z);
                            let v1 = Vector3::new(v1.x, v1.y, v1.z);
                            let v2 = Vector3::new(v2.x, v2.y, v2.z);
                            let n = normal_v(v0, v1, v2).normalize();
                            let mut v0 = v0.to_homogeneous();
                            v0.w = 1.0;
                            let mut v1 = v1.to_homogeneous();
                            v1.w = 1.0;
                            let mut v2 = v2.to_homogeneous();
                            v2.w = 1.0;
                            let v0 = viewport * perspective_m * model_view * v0;
                            let v1 = viewport * perspective_m * model_view * v1;
                            let v2 = viewport * perspective_m * model_view * v2;
                            let v0 = Vector3::new(v0.x / v0.w, v0.y / v0.w, v0.z / v0.w);
                            let v1 = Vector3::new(v1.x / v1.w, v1.y / v1.w, v1.z / v1.w);
                            let v2 = Vector3::new(v2.x / v2.w, v2.y / v2.w, v2.z / v2.w);
                            draw::triangle_z(&mut buffer, &mut z_buffer, v0, v1, v2, |p| {
                                let l = point_light.position - p;
                                let d = l.norm();
                                // println!("{},{},{}",point_light.position,p,d);
                                let l = l.normalize();
                                let intensity = nalgebra::base::Matrix::dot(&n, &l);
                                if intensity > 0. {
                                    // 距离太大,消减一点
                                    let intensity = intensity * dist(point_light.decay, d/10., 400.);
                                    // println!("{d},{intensity}");
                                    Color::White.mul(intensity)
                                } else {
                                    Color::Blue.mul(0.1)
                                }
                            });
                            // draw::triangle_line(
                            //     &mut buffer,
                            //     [v0.x, v0.y].into(),
                            //     [v1.x, v1.y].into(),
                            //     [v2.x, v2.y].into(),
                            //     Color::Red,
                            // );
                        }
                    }
                }
            }
        }

        while window.is_open() {
            update_with_buffer(&mut window, &buffer);
        }
    }
}
