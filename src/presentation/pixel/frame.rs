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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRun {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub color: Rgba8,
}

impl PixelFrame {
    fn assert_storage_invariant(&self) {
        let expected_len = usize::from(self.width) * usize::from(self.height);
        assert_eq!(
            self.pixels.len(),
            expected_len,
            "PixelFrame invariant violated: pixels.len() must equal width * height"
        );
    }

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
        self.assert_storage_invariant();
        let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.pixels[idx] = color;
    }

    pub fn opaque_pixel_count(&self) -> usize {
        self.assert_storage_invariant();
        self.pixels.iter().filter(|pixel| pixel.a > 0).count()
    }

    pub fn changed_pixel_count(&self, other: &Self) -> usize {
        self.assert_storage_invariant();
        other.assert_storage_invariant();
        assert_eq!((self.width, self.height), (other.width, other.height));
        self.pixels
            .iter()
            .zip(&other.pixels)
            .filter(|(a, b)| a != b)
            .count()
    }

    pub fn opaque_bounds(&self) -> Option<PixelBounds> {
        self.assert_storage_invariant();
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

    pub fn alpha_bounds(&self, min_alpha: u8) -> Option<PixelBounds> {
        self.assert_storage_invariant();
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0_u16;
        let mut max_y = 0_u16;
        let mut found = false;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
                if self.pixels[idx].a < min_alpha {
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

pub fn pixel_runs(frame: &PixelFrame) -> Vec<PixelRun> {
    frame.assert_storage_invariant();
    let mut runs = Vec::new();
    for y in 0..frame.height {
        let mut x = 0;
        while x < frame.width {
            let idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
            let color = frame.pixels[idx];
            if color.a == 0 {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < frame.width {
                let next_idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
                if frame.pixels[next_idx] != color {
                    break;
                }
                x += 1;
            }
            runs.push(PixelRun { x: start, y, width: x - start, color });
        }
    }
    runs
}
