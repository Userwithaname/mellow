use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A color in sRGB colorspace, intended for display
#[must_use]
#[derive(Clone, Copy)]
pub struct ColorSRGB {
    pub channels: [u8; 3],
}
impl From<[u8; 3]> for ColorSRGB {
    #[inline]
    fn from(value: [u8; 3]) -> Self {
        ColorSRGB { channels: value }
    }
}
impl ColorSRGB {
    /// Returns a plain white color
    #[inline]
    pub fn white() -> Self {
        ColorSRGB::from([255, 255, 255])
    }
    /// Runs the given closure per each color channel
    ///
    /// # Note
    /// For more accurate color processing, convert to `ColorLinear`
    /// first (using `to_linear`), then back using `ColorLinear::to_srgb`
    #[inline]
    pub fn per_channel<F: FnMut(&mut u8)>(&mut self, mut f: F) {
        for c in &mut self.channels {
            f(c);
        }
    }
    /// Returns the color components as a tuple (_r_, _g_, _b_)
    #[inline]
    #[must_use]
    pub const fn as_tuple(&self) -> (u8, u8, u8) {
        (self.channels[0], self.channels[1], self.channels[2])
    }
    /// Converts the color to `ColorLinear`, intended for processing
    #[inline]
    pub fn to_linear(self) -> ColorLinear {
        ColorLinear::from(self.channels.map(|c| (c as f64 / 255.0).powf(2.2)))
    }
}

/// A color in linear colorspace, intended for processing
#[must_use]
#[derive(Clone, Copy)]
pub struct ColorLinear {
    pub channels: [f64; 3],
}
impl From<[f64; 3]> for ColorLinear {
    #[inline]
    fn from(value: [f64; 3]) -> Self {
        ColorLinear { channels: value }
    }
}
impl ColorLinear {
    /// Runs the given closure per each color channel
    #[inline]
    pub fn per_channel<F: FnMut(&mut f64)>(&mut self, mut f: F) {
        for c in &mut self.channels {
            f(c);
        }
    }
    /// Raises a number to an integer power (runs `f64::powi(n)` for each color component)
    #[inline]
    pub fn powi(mut self, n: i32) -> Self {
        self.per_channel(|c| *c = c.powi(n));
        self
    }
    /// Returns the perceived color luminance
    /// (<https://stackoverflow.com/questions/596216/formula-to-determine-perceived-brightness-of-rgb-color/596243#596243>)
    #[inline]
    #[must_use]
    pub const fn lum(&self) -> f64 {
        let c = self.channels;
        c[0].mul_add(0.2126, c[1].mul_add(0.7152, c[2] * 0.0722))
        // c[0].mul_add(0.299, c[1].mul_add(0.587, c[2] * 0.114))
    }
    /// Converts the color to sRGB
    #[inline]
    pub fn to_srgb(self) -> ColorSRGB {
        ColorSRGB::from(self.channels.map(|c| (c.powf(1.0 / 2.2) * 255.0) as u8))
    }
    /// Converts the color to sRGB using a rough approximation
    ///
    /// Note that pure white and pure black are not possible with this function
    #[inline]
    pub fn to_srgb_approx(self) -> ColorSRGB {
        ColorSRGB::from(self.channels.map(|c| {
            (((1.0 - c).mul_add(-(1.0 - c), 0.8) + 0.4) / 1.25 * 255.0) as u8 // (comment for formatting)
        }))
    }
}

// The rest of this file is just math trait implementations
//
// I've opted to use algebraic float operations, since the color
// values will likely be similar enough to not cause precision
// issues if the compiler chooses to reorder them

impl Mul<f64> for ColorLinear {
    type Output = Self;

    #[inline]
    fn mul(mut self, rhs: f64) -> Self::Output {
        self.per_channel(|lhs| *lhs = lhs.algebraic_mul(rhs));
        self
    }
}
impl Mul<ColorLinear> for f64 {
    type Output = ColorLinear;

    #[inline]
    fn mul(self, mut rhs: ColorLinear) -> Self::Output {
        rhs.per_channel(|rhs| *rhs = self.algebraic_mul(*rhs));
        rhs
    }
}
impl MulAssign<f64> for ColorLinear {
    #[inline]
    fn mul_assign(&mut self, rhs: f64) {
        self.per_channel(|lhs| *lhs = lhs.algebraic_mul(rhs));
    }
}

impl Div<f64> for ColorLinear {
    type Output = Self;

    #[inline]
    fn div(mut self, rhs: f64) -> Self::Output {
        self.per_channel(|lhs| *lhs = lhs.algebraic_div(rhs));
        self
    }
}
impl Div<ColorLinear> for f64 {
    type Output = ColorLinear;

    #[inline]
    fn div(self, mut rhs: ColorLinear) -> Self::Output {
        rhs.per_channel(|rhs| *rhs = self.algebraic_div(*rhs));
        rhs
    }
}
impl DivAssign<f64> for ColorLinear {
    #[inline]
    fn div_assign(&mut self, rhs: f64) {
        self.per_channel(|lhs| *lhs = lhs.algebraic_div(rhs));
    }
}

impl Add<f64> for ColorLinear {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: f64) -> Self::Output {
        self.per_channel(|lhs| *lhs = lhs.algebraic_add(rhs));
        self
    }
}
impl Add<ColorLinear> for f64 {
    type Output = ColorLinear;

    #[inline]
    fn add(self, mut rhs: ColorLinear) -> Self::Output {
        rhs.per_channel(|rhs| *rhs = self.algebraic_add(*rhs));
        rhs
    }
}
impl AddAssign<f64> for ColorLinear {
    #[inline]
    fn add_assign(&mut self, rhs: f64) {
        self.per_channel(|lhs| *lhs = lhs.algebraic_add(rhs));
    }
}

impl Sub<f64> for ColorLinear {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: f64) -> Self::Output {
        self.per_channel(|lhs| *lhs = lhs.algebraic_sub(rhs));
        self
    }
}
impl Sub<ColorLinear> for f64 {
    type Output = ColorLinear;

    #[inline]
    fn sub(self, mut rhs: ColorLinear) -> Self::Output {
        rhs.per_channel(|rhs| *rhs = self.algebraic_sub(*rhs));
        rhs
    }
}
impl SubAssign<f64> for ColorLinear {
    #[inline]
    fn sub_assign(&mut self, rhs: f64) {
        self.per_channel(|lhs| *lhs = lhs.algebraic_sub(rhs));
    }
}
