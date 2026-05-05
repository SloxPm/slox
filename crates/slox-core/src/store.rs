use std::path::PathBuf;
use std::sync::OnceLock;

static SLOX_STORE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) struct StorePaths {
    pub(crate) base: PathBuf,
}

impl StorePaths {
    pub(crate) fn default() -> Self {
        Self {
            base: default_store().clone(),
        }
    }

    pub(crate) fn env_dir(&self, path: &str) -> PathBuf {
        self.base.join("env").join(path)
    }

    pub(crate) fn root_bin_dir(&self) -> PathBuf {
        self.base.join("root").join("bin")
    }

    pub(crate) fn shim_bin_dir(&self) -> PathBuf {
        self.base.join("bin")
    }

    pub(crate) fn cache_root(&self) -> PathBuf {
        self.base.join("tmp")
    }

    pub(crate) fn active_env_file(&self) -> PathBuf {
        self.base.join("active-env")
    }

    pub(crate) fn env_bin_dir(&self, path: &str) -> PathBuf {
        self.env_dir(path).join("bin")
    }
}

fn default_store() -> &'static PathBuf {
    SLOX_STORE.get_or_init(|| {
        let mut home = home::home_dir().expect("failed to find home path");
        home.push(".slox");
        home
    })
}
