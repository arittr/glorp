#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppConfig {
    pub cache_read_weight: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cache_read_weight: 0.03,
        }
    }
}
