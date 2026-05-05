use crate::env::{active_bin_dir, sync_active_shims};
use crate::store::StorePaths;
use git2::Repository;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::{Table, Value};

#[derive(Debug)]
pub(crate) enum Pkg {
    GitHub { user: String, repo: String },
    Regit { pkg: String },
}

pub(crate) struct PackageInstall {
    pub(crate) package_name: String,
    pub(crate) installed_binary: PathBuf,
    pub(crate) shim_binary: PathBuf,
}

pub(crate) struct PackageRemove {
    pub(crate) package_name: String,
    pub(crate) removed_binary: PathBuf,
    pub(crate) shim_binary: PathBuf,
    pub(crate) removed: bool,
}

pub(crate) struct BuildConfig {
    pub(crate) name: String,
    pub(crate) script: PathBuf,
    pub(crate) binary: PathBuf,
}

pub(crate) fn parse_pkg(path: &str) -> Result<Pkg, String> {
    let (pkg, from) = path
        .rsplit_once('@')
        .ok_or_else(|| "package must look like `user/repo@github`".to_string())?;

    match from {
        "github" => {
            let (user, repo) = pkg
                .split_once('/')
                .ok_or_else(|| "github package must look like `user/repo@github`".to_string())?;

            if user.is_empty() || repo.is_empty() {
                return Err("github package must include both user and repo".to_string());
            }

            Ok(Pkg::GitHub {
                user: user.to_string(),
                repo: repo.to_string(),
            })
        }
        "sloxpkgs" => Ok(Pkg::Regit {
            pkg: pkg.to_string(),
        }),
        _ => Err(format!("unsupported package source `{from}`")),
    }
}

pub(crate) fn load_build_config(
    repo_dir: &Path,
    package_name: &str,
) -> Result<BuildConfig, String> {
    let config_path = repo_dir.join("build.toml");
    if !config_path.is_file() {
        return Ok(BuildConfig {
            name: package_name.to_string(),
            script: PathBuf::from("build"),
            binary: PathBuf::from("bin").join(package_name),
        });
    }

    let contents = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "failed to read build config `{}`: {error}",
            config_path.display()
        )
    })?;
    let value = contents.parse::<Table>().map_err(|error| {
        format!(
            "failed to parse build config `{}`: {error}",
            config_path.display()
        )
    })?;

    let name = value
        .get("name")
        .map(value_to_string)
        .transpose()?
        .unwrap_or_else(|| package_name.to_string());
    let script = value
        .get("script")
        .map(value_to_path)
        .transpose()?
        .unwrap_or_else(|| PathBuf::from("build"));
    let binary = value
        .get("binary")
        .map(value_to_path)
        .transpose()?
        .unwrap_or_else(|| PathBuf::from("bin").join(package_name));

    Ok(BuildConfig {
        name,
        script,
        binary,
    })
}

fn value_to_path(value: &Value) -> Result<PathBuf, String> {
    value
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| "expected a string path".to_string())
}

fn value_to_string(value: &Value) -> Result<String, String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "expected a string".to_string())
}

pub(crate) fn run_build_script(repo_dir: &Path, config: &BuildConfig) -> Result<(), String> {
    let build_script = repo_dir.join(&config.script);
    if !build_script.is_file() {
        return Err(format!(
            "expected build script at `{}`",
            build_script.display()
        ));
    }

    let output = Command::new("bash")
        .arg(&config.script)
        .current_dir(repo_dir)
        .output()
        .map_err(|error| {
            format!(
                "failed to run build script `{}`: {error}",
                build_script.display()
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "build script exited with a non-zero status".to_string()
    };

    Err(details)
}

pub(crate) fn install_built_binary(
    package_dir: &Path,
    built_binary_relpath: &Path,
    install_name: &str,
    root_bin_dir: &Path,
) -> Result<PathBuf, String> {
    let built_binary = package_dir.join(built_binary_relpath);
    if !built_binary.is_file() {
        return Err(format!(
            "expected built binary at `{}`",
            built_binary.display()
        ));
    }

    fs::create_dir_all(root_bin_dir).map_err(|error| {
        format!(
            "failed to create root bin dir `{}`: {error}",
            root_bin_dir.display()
        )
    })?;

    let installed_binary = root_bin_dir.join(install_name);
    fs::copy(&built_binary, &installed_binary).map_err(|error| {
        format!(
            "failed to install `{}` to `{}`: {error}",
            built_binary.display(),
            installed_binary.display()
        )
    })?;

    Ok(installed_binary)
}

fn clone_or_open_repo(url: &str, repo_dir: &Path) -> Result<Repository, String> {
    if repo_dir.exists() {
        return Repository::open(repo_dir).map_err(|error| {
            format!(
                "failed to open cached repository `{}`: {error}",
                repo_dir.display()
            )
        });
    }

    let repo = Repository::clone(url, repo_dir).map_err(|error| {
        format!(
            "failed to clone `{url}` into `{}`: {error}",
            repo_dir.display()
        )
    })?;
    Ok(repo)
}

fn install_package_from_dir(
    store: &StorePaths,
    package_dir: &Path,
    package_name: &str,
) -> Result<PackageInstall, String> {
    if !package_dir.is_dir() {
        return Err(format!(
            "package directory `{}` does not exist",
            package_dir.display()
        ));
    }

    let build_config = load_build_config(package_dir, package_name)
        .map_err(|details| format!("build config failed for `{package_name}`: {details}"))?;
    run_build_script(package_dir, &build_config)
        .map_err(|details| format!("build failed for `{package_name}`: {details}"))?;
    let install_dir = active_bin_dir(store)?;
    let installed_binary = install_built_binary(
        package_dir,
        &build_config.binary,
        &build_config.name,
        &install_dir,
    )?;
    sync_active_shims(store)?;
    Ok(PackageInstall {
        package_name: build_config.name.clone(),
        shim_binary: store.shim_bin_dir().join(&build_config.name),
        installed_binary,
    })
}

fn install_package_from_repo(
    store: &StorePaths,
    url: &str,
    repo_name: &str,
) -> Result<PackageInstall, String> {
    let cache_root = store.cache_root();
    fs::create_dir_all(&cache_root).map_err(|error| {
        format!(
            "failed to create package cache `{}`: {error}",
            cache_root.display()
        )
    })?;

    let repo_dir = cache_root.join(repo_name);
    let _repo = clone_or_open_repo(url, &repo_dir)?;
    install_package_from_dir(store, &repo_dir, repo_name)
}

fn install_sloxpkg_from_registry(store: &StorePaths, pkg: &str) -> Result<PackageInstall, String> {
    let cache_root = store.cache_root();
    fs::create_dir_all(&cache_root).map_err(|error| {
        format!(
            "failed to create package cache `{}`: {error}",
            cache_root.display()
        )
    })?;

    let registry_url = "https://github.com/SloxPm/std-pkg";
    let registry_dir = cache_root.join("std-pkg");
    let registry_repo = clone_or_open_repo(registry_url, &registry_dir)?;

    if let Ok(mut submodule) = registry_repo.find_submodule(pkg) {
        let submodule_url = submodule.url().unwrap_or("unknown url").to_string();
        submodule.update(true, None).map_err(|error| {
            format!("failed to initialize sloxpkgs package `{pkg}` from `{submodule_url}`: {error}")
        })?;
    }

    let package_dir = registry_dir.join(pkg);
    if !package_dir.is_dir() {
        return Err(format!(
            "package `{pkg}` was not found in sloxpkgs registry `{registry_url}`"
        ));
    }

    install_package_from_dir(store, &package_dir, pkg)
}

pub(crate) fn download(store: &StorePaths, pkg: &Pkg) -> Result<PackageInstall, String> {
    match pkg {
        Pkg::GitHub { user, repo } => {
            let url = format!("https://github.com/{user}/{repo}");
            install_package_from_repo(store, &url, repo)
        }
        Pkg::Regit { pkg } => install_sloxpkg_from_registry(store, pkg),
    }
}

fn package_name_from_remove_arg(store: &StorePaths, path: &str) -> Result<String, String> {
    if !path.contains('@') {
        return Ok(path.to_string());
    }

    let pkg = parse_pkg(path)?;
    match &pkg {
        Pkg::GitHub { user, repo } => {
            let url = format!("https://github.com/{user}/{repo}");
            let repo_dir = store.cache_root().join(repo);
            let _repo = clone_or_open_repo(&url, &repo_dir)?;
            Ok(load_build_config(&repo_dir, repo)?.name)
        }
        Pkg::Regit { pkg } => {
            let registry_dir = store.cache_root().join("std-pkg");
            let registry_url = "https://github.com/SloxPm/std-pkg";
            let registry_repo = clone_or_open_repo(registry_url, &registry_dir)?;

            if let Ok(mut submodule) = registry_repo.find_submodule(pkg) {
                let submodule_url = submodule.url().unwrap_or("unknown url").to_string();
                submodule.update(true, None).map_err(|error| {
                    format!(
                        "failed to initialize sloxpkgs package `{pkg}` from `{submodule_url}`: {error}"
                    )
                })?;
            }

            Ok(load_build_config(&registry_dir.join(pkg), pkg)?.name)
        }
    }
}

pub(crate) fn remove(store: &StorePaths, path: &str) -> Result<PackageRemove, String> {
    let package_name = package_name_from_remove_arg(store, path)?;
    let installed_binary = active_bin_dir(store)?.join(&package_name);
    let shim_binary = store.shim_bin_dir().join(&package_name);

    if !installed_binary.exists() {
        return Ok(PackageRemove {
            package_name,
            removed_binary: installed_binary,
            shim_binary,
            removed: false,
        });
    }

    fs::remove_file(&installed_binary).map_err(|error| {
        format!(
            "failed to remove package binary `{}`: {error}",
            installed_binary.display()
        )
    })?;
    sync_active_shims(store)?;

    Ok(PackageRemove {
        package_name,
        removed_binary: installed_binary,
        shim_binary,
        removed: true,
    })
}
