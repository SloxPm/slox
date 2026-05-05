use crate::store::StorePaths;
use slox_shim::Shim;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;


pub(crate) struct RemoveEnvResult {
    pub(crate) env_dir: PathBuf,
    pub(crate) removed: bool,
    pub(crate) cleared_active: bool,
}

pub(crate) fn add_env(store: &StorePaths, path: &str) -> Result<PathBuf, String> {
    let env_dir = store.env_dir(path);
    fs::create_dir_all(store.env_bin_dir(path))
        .map_err(|error| format!("failed to create env `{}`: {error}", env_dir.display()))?;

    Ok(env_dir)
}

pub(crate) fn remove_env(store: &StorePaths, path: &str) -> Result<RemoveEnvResult, String> {
    let env_dir = store.env_dir(path);
    if !env_dir.exists() {
        return Ok(RemoveEnvResult {
            env_dir,
            removed: false,
            cleared_active: false,
        });
    }

    fs::remove_dir_all(&env_dir)
        .map_err(|error| format!("failed to remove env `{}`: {error}", env_dir.display()))?;

    let mut cleared_active = false;
    if active_env_name(store)?.as_deref() == Some(path) {
        clear_active_env(store)?;
        sync_active_shims(store)?;
        cleared_active = true;
    }

    Ok(RemoveEnvResult {
        env_dir,
        removed: true,
        cleared_active,
    })
}

pub(crate) fn set_env(store: &StorePaths, path: &str) -> Result<PathBuf, String> {
    let env_dir = store.env_dir(path);
    if !env_dir.is_dir() {
        return Err(format!(
            "environment `{path}` does not exist. Run `slox env add {path}` first."
        ));
    }

    fs::create_dir_all(&store.base).map_err(|error| {
        format!(
            "failed to prepare store `{}`: {error}",
            store.base.display()
        )
    })?;
    fs::write(store.active_env_file(), format!("{path}\n"))
        .map_err(|error| format!("failed to set active environment `{path}`: {error}"))?;
    sync_active_shims(store)?;

    Ok(store.env_bin_dir(path))
}

pub(crate) fn get_env(store:&StorePaths, path:&str) -> Result<PathBuf, String> {
    let env_dir = store.env_dir(path);
    if !env_dir.is_dir() {
        return Err(format!(
            "environment `{path}` does not exist. Run `slox env add {path}` first."
        ));
    }
    Ok(store.env_bin_dir(path))    
}

fn clear_active_env(store: &StorePaths) -> Result<(), String> {
    let active_env_file = store.active_env_file();
    if active_env_file.exists() {
        fs::remove_file(&active_env_file).map_err(|error| {
            format!(
                "failed to clear active environment `{}`: {error}",
                active_env_file.display()
            )
        })?;
    }
    Ok(())
}

pub(crate) fn active_env_name(store: &StorePaths) -> Result<Option<String>, String> {
    let active_env_file = store.active_env_file();
    if !active_env_file.exists() {
        return Ok(None);
    }

    let name = fs::read_to_string(&active_env_file).map_err(|error| {
        format!(
            "failed to read active environment `{}`: {error}",
            active_env_file.display()
        )
    })?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(trimmed.to_string()))
}

pub(crate) fn active_bin_dir(store: &StorePaths) -> Result<PathBuf, String> {
    match active_env_name(store)? {
        Some(env_name) => {
            let env_dir = store.env_dir(&env_name);
            if !env_dir.is_dir() {
                return Err(format!(
                    "active environment `{env_name}` is missing. Run `plox env add {env_name}` or set another one."
                ));
            }

            Ok(store.env_bin_dir(&env_name))
        }
        None => Ok(store.root_bin_dir()),
    }
}

pub(crate) fn sync_active_shims(store: &StorePaths) -> Result<(), String> {
    let source_bin_dir = active_bin_dir(store)?;
    let shim_bin_dir = store.shim_bin_dir();

    fs::create_dir_all(&shim_bin_dir).map_err(|error| {
        format!(
            "failed to create shim bin dir `{}`: {error}",
            shim_bin_dir.display()
        )
    })?;

    for entry in fs::read_dir(&shim_bin_dir).map_err(|error| {
        format!(
            "failed to read shim bin dir `{}`: {error}",
            shim_bin_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect shim bin dir `{}`: {error}",
                shim_bin_dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| {
                format!("failed to remove stale shim `{}`: {error}", path.display())
            })?;
        }
    }

    if !source_bin_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(&source_bin_dir).map_err(|error| {
        format!(
            "failed to read active bin dir `{}`: {error}",
            source_bin_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect active bin dir `{}`: {error}",
                source_bin_dir.display()
            )
        })?;
        let source_path = entry.path();
        if !source_path.is_file() {
            continue;
        }

        let shim_name = entry.file_name();
        let shim_path = shim_bin_dir.join(&shim_name);
        let shim_contents = Shim::new(&source_path.to_string_lossy()).generate();
        fs::write(&shim_path, shim_contents)
            .map_err(|error| format!("failed to write shim `{}`: {error}", shim_path.display()))?;

        let mut permissions = fs::metadata(&shim_path)
            .map_err(|error| format!("failed to inspect shim `{}`: {error}", shim_path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim_path, permissions).map_err(|error| {
            format!(
                "failed to mark shim `{}` executable: {error}",
                shim_path.display()
            )
        })?;
    }

    Ok(())
}
