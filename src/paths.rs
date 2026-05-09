use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub usage_db: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> crate::error::Result<Self> {
        if let Some(dir) = std::env::var_os("GLORP_CONFIG_DIR") {
            return Ok(Self::from_config_dir(PathBuf::from(dir)));
        }

        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            crate::error::GlorpError::Message(
                "HOME is not set; set GLORP_CONFIG_DIR to choose a config directory".into(),
            )
        })?;
        Ok(Self::from_home_dir(home))
    }

    pub fn from_home_dir(home: PathBuf) -> Self {
        Self::from_config_dir(home.join(".config").join("glorp"))
    }

    pub fn from_config_dir(config_dir: PathBuf) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            state_file: config_dir.join("state.json"),
            usage_db: config_dir.join("usage.sqlite"),
            config_dir,
        }
    }

    pub fn ensure(&self) -> crate::error::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        Ok(())
    }
}
