use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub token_enc: String,
    #[serde(default)]
    pub forum_id: Option<u64>,
}

impl Config {
    pub fn token(&self) -> Option<String> {
        if self.token_enc.is_empty() {
            None
        } else {
            crate::secret::open(&self.token_enc)
        }
    }

    pub fn set_token(&mut self, plain: &str) {
        self.token_enc = crate::secret::seal(plain);
    }
}

fn dir() -> PathBuf {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return PathBuf::from(x).join("lzt");
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("lzt")
}

fn path() -> PathBuf {
    dir().join("config.json")
}

fn pretty(p: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        if let Ok(rest) = p.strip_prefix(PathBuf::from(home)) {
            return format!("~/{}", rest.display());
        }
    }
    p.display().to_string()
}

pub fn load() -> Config {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &Config) -> std::io::Result<String> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

    let d = dir();
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&d)?;

    let p = path();
    let data = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&p)?;
    f.write_all(data.as_bytes())?;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;

    Ok(pretty(&p))
}
