pub mod cli;

use crate::cli::{ActivateCommand, Commands, EnvCommand, PkgCommand};
use git2::Repository;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static SLOX_STORE: OnceLock<PathBuf> = OnceLock::new();

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[38;5;196m";
const GREEN: &str = "\x1b[38;5;82m";
const YELLOW: &str = "\x1b[38;5;220m";
const CYAN: &str = "\x1b[38;5;51m";
const PURPLE: &str = "\x1b[38;5;141m";
const RAINBOW: [&str; 4] = [
    "\x1b[38;5;197m",
    "\x1b[38;5;214m",
    "\x1b[38;5;226m",
    "\x1b[38;5;45m",
];

#[derive(Debug, Clone)]
struct StorePaths {
    base: PathBuf,
}

impl StorePaths {
    fn default() -> Self {
        Self {
            base: default_store().clone(),
        }
    }

    fn env_dir(&self, path: &str) -> PathBuf {
        self.base.join("env").join(path)
    }

    fn root_bin_dir(&self) -> PathBuf {
        self.base.join("root").join("bin")
    }

    fn cache_root(&self) -> PathBuf {
        self.base.join("tmp")
    }

    fn active_env_file(&self) -> PathBuf {
        self.base.join("active-env")
    }

    fn env_bin_dir(&self, path: &str) -> PathBuf {
        self.env_dir(path).join("bin")
    }
}

#[derive(Debug)]
enum Pkg {
    GitHub { user: String, repo: String },
    Regit { pkg: String },
}

fn default_store() -> &'static PathBuf {
    SLOX_STORE.get_or_init(|| {
        let mut home = home::home_dir().expect("failed to find home path");
        home.push(".slox");
        home
    })
}

fn rainbow_prefix() -> String {
    let mut prefix = String::new();
    for (index, ch) in "SLOX".chars().enumerate() {
        prefix.push_str(RAINBOW[index % RAINBOW.len()]);
        prefix.push(ch);
    }
    prefix.push_str(RESET);
    prefix
}

fn print_status(label: &str, color: &str, message: &str) {
    eprintln!("{color}{label:<5}{RESET} {message}");
}

fn print_sparkle(message: &str) {
    eprintln!("{} {GREEN}{message}{RESET}", rainbow_prefix());
}

pub fn report_error(error: &str) {
    print_status("ERR", RED, error);
}

pub fn run(command: Commands) -> Result<(), String> {
    run_with_store(command, &StorePaths::default())
}

fn run_with_store(command: Commands, store: &StorePaths) -> Result<(), String> {
    match command {
        Commands::Env { command } => handle_env(store, command),
        Commands::Activate { command } => {
            println!("{}", activation_script(store, command)?);
            Ok(())
        }
        Commands::Pkg { command } => handle_pkg(store, command),
    }
}

fn handle_env(store: &StorePaths, cmd: EnvCommand) -> Result<(), String> {
    match cmd {
        EnvCommand::Add { path } => add_env(store, &path),
        EnvCommand::Remove { path } => remove_env(store, &path),
        EnvCommand::Set { path } => set_env(store, &path),
    }
}

fn handle_pkg(store: &StorePaths, cmd: PkgCommand) -> Result<(), String> {
    match cmd {
        PkgCommand::Add { path } => {
            let pkg = parse_pkg(&path)?;
            download(store, &pkg)
        }
        PkgCommand::Remove { path } => {
            print_status(
                "WARN",
                YELLOW,
                &format!("Package removal is not implemented yet for `{path}`."),
            );
            Ok(())
        }
    }
}

fn add_env(store: &StorePaths, path: &str) -> Result<(), String> {
    let env_dir = store.env_dir(path);
    fs::create_dir_all(store.env_bin_dir(path))
        .map_err(|error| format!("failed to create env `{}`: {error}", env_dir.display()))?;

    print_sparkle(&format!("Environment ready at {}", env_dir.display()));
    Ok(())
}

fn remove_env(store: &StorePaths, path: &str) -> Result<(), String> {
    let env_dir = store.env_dir(path);
    if !env_dir.exists() {
        print_status(
            "WARN",
            YELLOW,
            &format!("Environment `{}` does not exist, nothing to remove.", path),
        );
        return Ok(());
    }

    fs::remove_dir_all(&env_dir)
        .map_err(|error| format!("failed to remove env `{}`: {error}", env_dir.display()))?;

    if active_env_name(store)?.as_deref() == Some(path) {
        clear_active_env(store)?;
        print_status(
            "INFO",
            CYAN,
            &format!("Cleared active environment `{path}` and fell back to root/bin."),
        );
    }

    print_sparkle(&format!("Environment removed from {}", env_dir.display()));
    Ok(())
}

fn set_env(store: &StorePaths, path: &str) -> Result<(), String> {
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

    print_sparkle(&format!(
        "Active environment set to {path} -> {}",
        store.env_bin_dir(path).display()
    ));
    Ok(())
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

fn active_env_name(store: &StorePaths) -> Result<Option<String>, String> {
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

fn active_bin_dir(store: &StorePaths) -> Result<PathBuf, String> {
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

fn activation_script(store: &StorePaths, cmd: ActivateCommand) -> Result<String, String> {
    let mut bin_path = active_bin_dir(store)?.to_string_lossy().to_string();
    if !bin_path.ends_with('/') {
        bin_path.push('/');
    }

    match cmd {
        ActivateCommand::Sh | ActivateCommand::Bash | ActivateCommand::Zsh => {
            Ok(format!(r#"export PATH="$PATH:{}""#, bin_path))
        }
        ActivateCommand::Nu => Ok(format!(
            r#"$env.PATH = ($env.PATH | split row (char esep) | append "{}")"#,
            bin_path
        )),
    }
}

fn parse_pkg(path: &str) -> Result<Pkg, String> {
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

fn run_build_script(repo_dir: &Path) -> Result<(), String> {
    let build_script = repo_dir.join("build");
    if !build_script.is_file() {
        return Err(format!(
            "expected build script at `{}`",
            build_script.display()
        ));
    }

    print_status(
        "BUILD",
        PURPLE,
        &format!("Running {}", build_script.display()),
    );
    let output = Command::new("bash")
        .arg("build")
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

fn install_built_binary(
    repo_dir: &Path,
    repo: &str,
    root_bin_dir: &Path,
) -> Result<PathBuf, String> {
    let built_binary = repo_dir.join("bin").join(repo);
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

    let installed_binary = root_bin_dir.join(repo);
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
        print_status(
            "INFO",
            CYAN,
            &format!("Using cached repository at {}", repo_dir.display()),
        );
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

    print_sparkle(&format!("Cloned {url} into {}", repo_dir.display()));
    Ok(repo)
}

fn install_package_from_dir(
    store: &StorePaths,
    package_dir: &Path,
    package_name: &str,
) -> Result<PathBuf, String> {
    if !package_dir.is_dir() {
        return Err(format!(
            "package directory `{}` does not exist",
            package_dir.display()
        ));
    }

    run_build_script(package_dir)
        .map_err(|details| format!("build failed for `{package_name}`: {details}"))?;
    let install_dir = active_bin_dir(store)?;
    install_built_binary(package_dir, package_name, &install_dir)
}

fn install_package_from_repo(
    store: &StorePaths,
    url: &str,
    repo_name: &str,
) -> Result<PathBuf, String> {
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

fn install_sloxpkg_from_registry(store: &StorePaths, pkg: &str) -> Result<PathBuf, String> {
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

fn download(store: &StorePaths, pkg: &Pkg) -> Result<(), String> {
    match pkg {
        Pkg::GitHub { user, repo } => {
            let url = format!("https://github.com/{user}/{repo}");
            let installed_binary = install_package_from_repo(store, &url, repo)?;

            print_sparkle(&format!(
                "Installed {repo} to {}",
                installed_binary.display()
            ));
            Ok(())
        }
        Pkg::Regit { pkg } => {
            let installed_binary = install_sloxpkg_from_registry(store, pkg)?;

            print_sparkle(&format!(
                "Installed {pkg} to {}",
                installed_binary.display()
            ));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Pkg, StorePaths, activation_script, active_bin_dir, active_env_name, add_env,
        install_built_binary, parse_pkg, remove_env, run_build_script, set_env,
    };
    use crate::cli::ActivateCommand;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("slox-{name}-{unique}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    fn make_store() -> StorePaths {
        StorePaths {
            base: make_temp_dir("store"),
        }
    }

    #[test]
    fn parse_github_package() {
        let pkg = parse_pkg("devwon/slox@github").expect("github package should parse");
        match pkg {
            Pkg::GitHub { user, repo } => {
                assert_eq!(user, "devwon");
                assert_eq!(repo, "slox");
            }
            Pkg::Regit { .. } => panic!("expected github package"),
        }
    }

    #[test]
    fn parse_regit_package() {
        let pkg = parse_pkg("plur@sloxpkgs").expect("sloxpkgs package should parse");
        match pkg {
            Pkg::Regit { pkg } => assert_eq!(pkg, "plur"),
            Pkg::GitHub { .. } => panic!("expected sloxpkgs package"),
        }
    }

    #[test]
    fn reject_invalid_package_format() {
        let error = parse_pkg("invalid-package").expect_err("package parse should fail");
        assert!(error.contains("user/repo@github"));
    }

    #[test]
    fn activation_script_uses_root_bin_path() {
        let store = StorePaths {
            base: PathBuf::from("/tmp/slox-store"),
        };

        let script = activation_script(&store, ActivateCommand::Zsh)
            .expect("root activation script should render");
        assert_eq!(script, r#"export PATH="$PATH:/tmp/slox-store/root/bin/""#);
    }

    #[test]
    fn add_and_remove_env_under_store() {
        let store = make_store();
        let env_dir = store.env_dir("demo");

        add_env(&store, "demo").expect("env should be created");
        assert!(env_dir.is_dir());
        assert!(store.env_bin_dir("demo").is_dir());

        remove_env(&store, "demo").expect("env should be removed");
        assert!(!env_dir.exists());

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn set_env_changes_active_bin_dir_and_activation_script() {
        let store = make_store();
        add_env(&store, "demo").expect("env should be created");

        set_env(&store, "demo").expect("env should be set");
        assert_eq!(
            active_env_name(&store).expect("active env should be readable"),
            Some("demo".to_string())
        );
        assert_eq!(
            active_bin_dir(&store).expect("active bin dir should resolve"),
            store.env_bin_dir("demo")
        );

        let script = activation_script(&store, ActivateCommand::Bash)
            .expect("env activation script should render");
        assert_eq!(
            script,
            format!(
                r#"export PATH="$PATH:{}/""#,
                store.env_bin_dir("demo").display()
            )
        );

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn removing_active_env_clears_active_env_file() {
        let store = make_store();
        add_env(&store, "demo").expect("env should be created");
        set_env(&store, "demo").expect("env should be set");

        remove_env(&store, "demo").expect("env should be removed");
        assert_eq!(
            active_env_name(&store).expect("active env should be readable"),
            None
        );
        assert_eq!(
            active_bin_dir(&store).expect("root bin should be used"),
            store.root_bin_dir()
        );

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn build_and_install_github_package_layout() {
        let repo_name = "demo";
        let repo_dir = make_temp_dir("repo");
        let root_bin_dir = make_temp_dir("root-bin");

        fs::write(
            repo_dir.join("build"),
            format!(
                "#!/bin/sh\nmkdir -p bin\nprintf '#!/bin/sh\\necho installed\\n' > bin/{repo_name}\nchmod +x bin/{repo_name}\n"
            ),
        )
        .expect("failed to write build script");

        run_build_script(&repo_dir).expect("build script should succeed");
        let installed_binary = install_built_binary(&repo_dir, repo_name, &root_bin_dir)
            .expect("install should succeed");

        assert!(installed_binary.is_file());
        let contents =
            fs::read_to_string(installed_binary).expect("failed to read installed binary");
        assert!(contents.contains("installed"));

        fs::remove_dir_all(repo_dir).expect("failed to clean repo dir");
        fs::remove_dir_all(root_bin_dir).expect("failed to clean root bin dir");
    }

    #[test]
    fn install_target_follows_active_env_bin() {
        let store = make_store();
        let repo_name = "demo";
        let repo_dir = make_temp_dir("repo");

        add_env(&store, "demo-env").expect("env should be created");
        set_env(&store, "demo-env").expect("env should be set");

        fs::write(
            repo_dir.join("build"),
            format!(
                "#!/bin/sh\nmkdir -p bin\nprintf '#!/bin/sh\\necho active-env\\n' > bin/{repo_name}\nchmod +x bin/{repo_name}\n"
            ),
        )
        .expect("failed to write build script");

        run_build_script(&repo_dir).expect("build script should succeed");
        let install_dir = active_bin_dir(&store).expect("active bin dir should resolve");
        let installed_binary = install_built_binary(&repo_dir, repo_name, &install_dir)
            .expect("install should succeed");

        assert_eq!(
            installed_binary,
            store.env_bin_dir("demo-env").join(repo_name)
        );
        assert!(installed_binary.is_file());

        fs::remove_dir_all(repo_dir).expect("failed to clean repo dir");
        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }
}
