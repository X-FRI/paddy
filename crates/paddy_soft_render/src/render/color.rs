use std::ops::Mul;

#[derive(Clone, Copy)]
pub enum Color {
    White,
    Black,
    Red,
    Green,
    Blue,
    /// 品红色(紫红色)
    Magenta,
    Yellow,
    /// 青色
    Cyan,
    Random,
    Custom(u32),
    Custom2(u8, u8, u8, u8),
    /// 黑白调
    Black_White(f64),
}

impl Color {
    pub fn get_rbg(&self) -> (u8, u8, u8, u8) {
        let a: u32 = (*self).into();
        (
            (a / 0x01_00_00_00u32) as u8,
            (a / 0x01_00_00u32 % 0x01_00u32) as u8,
            (a / 0x01_00u32 % 0x01_00_00u32) as u8,
            (a % 0x01_00_00_00u32) as u8,
        )
    }
    pub fn mul(&self, rhs: f64) -> Self{
        let a = self.get_rbg();
        Color::Custom2(
            // (a.0 as f64 * rhs).round() as u8,
            a.0,//不对透明度做变动,虽然minifb对linux暂时不支持透明度,事实上我都不清楚 FF是透明还是00是透明
            (a.1 as f64 * rhs).round() as u8,
            (a.2 as f64 * rhs).round() as u8,
            (a.3 as f64 * rhs).round() as u8,
        )
    }
}

impl Into<u32> for Color {
    fn into(self) -> u32 {
        match self {
            Color::White => 0x00_FF_FF_FF,
            Color::Black => 0x0,

            Color::Red => 0x00_FF_00_00,
            Color::Green => 0x00_00_FF_00,
            Color::Blue => 0x00_00_00_FF,

            Color::Magenta => 0x00_FF_00_FF,
            Color::Yellow => 0x00_FF_FF_00,
            Color::Cyan => 0x00_00_FF_FF,

            Color::Random => rand::random::<u32>(),
            Color::Custom(v) => v,
            Color::Black_White(v) => {
                ((0xFFu32 as f64 * v).round() as u32
                    + (0xFFu32 as f64 * v).round() as u32 * 0x01_00u32
                    + (0xFFu32 as f64 * v).round() as u32 * 0x01_00_00u32
                    + (0xFFu32 as f64 * v).round() as u32 * 0x01_00_00_00u32)
            }
            Color::Custom2(a, b, c, d) => {
                (a as u32 * 0x01_00_00_00u32
                    + b as u32 * 0x01_00_00u32
                    + c as u32 * 0x01_00u32
                    + d as u32)
            }
        }
    }
}
