#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelViewport {
    pub logical_width: u16,
    pub logical_height: u16,
}

impl PixelViewport {
    pub const fn companion_default() -> Self {
        Self { logical_width: 96, logical_height: 96 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgba8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBounds {
    pub min_x: u16,
    pub min_y: u16,
    pub max_x: u16,
    pub max_y: u16,
}

impl PixelFrame {
    pub fn transparent(viewport: PixelViewport) -> Self {
        let len = usize::from(viewport.logical_width) * usize::from(viewport.logical_height);
        Self {
            width: viewport.logical_width,
            height: viewport.logical_height,
            pixels: vec![Rgba8::TRANSPARENT; len],
        }
    }

    pub fn set_pixel(&mut self, x: i16, y: i16, color: Rgba8) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u16;
        let y = y as u16;
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.pixels[idx] = color;
    }

    pub fn opaque_pixel_count(&self) -> usize {
        self.pixels.iter().filter(|pixel| pixel.a > 0).count()
    }

    pub fn changed_pixel_count(&self, other: &Self) -> usize {
        assert_eq!((self.width, self.height), (other.width, other.height));
        self.pixels
            .iter()
            .zip(&other.pixels)
            .filter(|(a, b)| a != b)
            .count()
    }

    pub fn opaque_bounds(&self) -> Option<PixelBounds> {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0_u16;
        let mut max_y = 0_u16;
        let mut found = false;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
                if self.pixels[idx].a == 0 {
                    continue;
                }
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }

        found.then_some(PixelBounds { min_x, min_y, max_x, max_y })
    }
}
