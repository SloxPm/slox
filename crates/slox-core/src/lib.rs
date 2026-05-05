mod activate;
mod env;
mod package;
mod store;
mod term;

use slox_cli::{Commands, EnvCommand, PkgCommand};
use store::StorePaths;
use term::{Progress, Report};

pub fn report_error(error: &str) {
    term::report_error(error);
}

pub fn run(command: Commands) -> Result<(), String> {
    run_with_store(command, &StorePaths::default())
}

fn run_with_store(command: Commands, store: &StorePaths) -> Result<(), String> {
    match command {
        Commands::Env { command } => {
            let progress = Progress::start("updating environment");
            match handle_env(store, command) {
                Ok(report) => {
                    progress.finish(report);
                    Ok(())
                }
                Err(error) => {
                    progress.fail();
                    Err(error)
                }
            }
        }
        Commands::Activate { command } => {
            println!("{}", activate::activation_script(store, command)?);
            Ok(())
        }
        Commands::Pkg { command } => {
            let progress = Progress::start("processing package");
            match handle_pkg(store, command) {
                Ok(report) => {
                    progress.finish(report);
                    Ok(())
                }
                Err(error) => {
                    progress.fail();
                    Err(error)
                }
            }
        }
    }
}

fn handle_env(store: &StorePaths, cmd: EnvCommand) -> Result<Report, String> {
    match cmd {
        EnvCommand::Add { path } => {
            let env_dir = env::add_env(store, &path)?;
            Ok(Report::new(format!("added env `{path}`"))
                .detail(format!("path: {}", env_dir.display()))
                .detail(format!("bin: {}", store.env_bin_dir(&path).display()))
                .detail(format!("shims: {}", store.shim_bin_dir().display())))
        }
        EnvCommand::Remove { path } => {
            let result = env::remove_env(store, &path)?;
            let summary = if result.removed {
                format!("removed env `{path}`")
            } else {
                format!("env `{path}` was already absent")
            };
            let mut report =
                Report::new(summary).detail(format!("path: {}", result.env_dir.display()));
            if result.cleared_active {
                report = report.detail("active env cleared; shims now point to root/bin");
            }
            Ok(report)
        }
        EnvCommand::Set { path } => {
            let active_bin = env::set_env(store, &path)?;
            Ok(Report::new(format!("activated env `{path}`"))
                .detail(format!("bin: {}", active_bin.display()))
                .detail(format!("shims: {}", store.shim_bin_dir().display())))
        }
    }
}

fn handle_pkg(store: &StorePaths, cmd: PkgCommand) -> Result<Report, String> {
    match cmd {
        PkgCommand::Add { path } => {
            let install = package::download(store, &package::parse_pkg(&path)?)?;
            Ok(
                Report::new(format!("added package `{}`", install.package_name))
                    .detail(format!("binary: {}", install.installed_binary.display()))
                    .detail(format!("shim: {}", install.shim_binary.display())),
            )
        }
        PkgCommand::Remove { path } => {
            let removed = package::remove(store, &path)?;
            let summary = if removed.removed {
                format!("removed package `{}`", removed.package_name)
            } else {
                format!("package `{}` was not installed", removed.package_name)
            };
            Ok(Report::new(summary)
                .detail(format!("binary: {}", removed.removed_binary.display()))
                .detail(format!("shim: {}", removed.shim_binary.display())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        activate::activation_script,
        env::{active_bin_dir, active_env_name, add_env, remove_env, set_env},
        package::{
            BuildConfig, Pkg, install_built_binary, parse_pkg, remove as remove_pkg,
            run_build_script,
        },
        store::StorePaths,
    };
    use crate::activate;
    use slox_cli::ActivateCommand;
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
        assert_eq!(
            script,
            concat!(
                "_slox_store='/tmp/slox-store'\n",
                "_slox_bin='/tmp/slox-store/bin'\n",
                "_slox_filtered_path=\n",
                "_slox_old_ifs=$IFS\n",
                "IFS=:\n",
                "for _slox_entry in $PATH; do\n",
                "  case \"$_slox_entry\" in\n",
                "    \"$_slox_store\"/bin|\"$_slox_store\"/bin/)\n",
                "      ;;\n",
                "    '')\n",
                "      ;;\n",
                "    *)\n",
                "      if [ -n \"$_slox_filtered_path\" ]; then\n",
                "        _slox_filtered_path=\"$_slox_filtered_path:$_slox_entry\"\n",
                "      else\n",
                "        _slox_filtered_path=\"$_slox_entry\"\n",
                "      fi\n",
                "      ;;\n",
                "  esac\n",
                "done\n",
                "IFS=$_slox_old_ifs\n",
                "if [ -n \"$_slox_filtered_path\" ]; then\n",
                "  export PATH=\"$_slox_bin:$_slox_filtered_path\"\n",
                "else\n",
                "  export PATH=\"$_slox_bin\"\n",
                "fi\n",
                "unset _slox_store _slox_bin _slox_filtered_path _slox_old_ifs _slox_entry"
            )
        );
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
                concat!(
                    "_slox_store={store}\n",
                    "_slox_bin={bin}\n",
                    "_slox_filtered_path=\n",
                    "_slox_old_ifs=$IFS\n",
                    "IFS=:\n",
                    "for _slox_entry in $PATH; do\n",
                    "  case \"$_slox_entry\" in\n",
                    "    \"$_slox_store\"/bin|\"$_slox_store\"/bin/)\n",
                    "      ;;\n",
                    "    '')\n",
                    "      ;;\n",
                    "    *)\n",
                    "      if [ -n \"$_slox_filtered_path\" ]; then\n",
                    "        _slox_filtered_path=\"$_slox_filtered_path:$_slox_entry\"\n",
                    "      else\n",
                    "        _slox_filtered_path=\"$_slox_entry\"\n",
                    "      fi\n",
                    "      ;;\n",
                    "  esac\n",
                    "done\n",
                    "IFS=$_slox_old_ifs\n",
                    "if [ -n \"$_slox_filtered_path\" ]; then\n",
                    "  export PATH=\"$_slox_bin:$_slox_filtered_path\"\n",
                    "else\n",
                    "  export PATH=\"$_slox_bin\"\n",
                    "fi\n",
                    "unset _slox_store _slox_bin _slox_filtered_path _slox_old_ifs _slox_entry"
                ),
                store = activate::shell_single_quote(&store.base.display().to_string()),
                bin = activate::shell_single_quote(&store.shim_bin_dir().display().to_string())
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

        let build_config = BuildConfig {
            name: repo_name.to_string(),
            script: PathBuf::from("build"),
            binary: PathBuf::from("bin").join(repo_name),
        };
        run_build_script(&repo_dir, &build_config).expect("build script should succeed");
        let installed_binary =
            install_built_binary(&repo_dir, &build_config.binary, repo_name, &root_bin_dir)
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

        let build_config = BuildConfig {
            name: repo_name.to_string(),
            script: PathBuf::from("build"),
            binary: PathBuf::from("bin").join(repo_name),
        };
        run_build_script(&repo_dir, &build_config).expect("build script should succeed");
        let install_dir = active_bin_dir(&store).expect("active bin dir should resolve");
        let installed_binary =
            install_built_binary(&repo_dir, &build_config.binary, repo_name, &install_dir)
                .expect("install should succeed");

        assert_eq!(
            installed_binary,
            store.env_bin_dir("demo-env").join(repo_name)
        );
        assert!(installed_binary.is_file());

        fs::remove_dir_all(repo_dir).expect("failed to clean repo dir");
        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn sync_active_shims_points_to_active_env_bins() {
        let store = make_store();
        add_env(&store, "demo").expect("env should be created");

        fs::create_dir_all(store.env_bin_dir("demo")).expect("env bin dir should exist");
        fs::write(
            store.env_bin_dir("demo").join("demo"),
            "#!/bin/sh\necho env\n",
        )
        .expect("binary should be written");

        set_env(&store, "demo").expect("env should be set");

        let shim_path = store.shim_bin_dir().join("demo");
        assert!(shim_path.is_file());
        let shim_contents = fs::read_to_string(&shim_path).expect("shim should be readable");
        assert!(
            shim_contents.contains(&store.env_bin_dir("demo").join("demo").display().to_string())
        );

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn clearing_active_env_refreshes_shims_to_root_bin() {
        let store = make_store();
        add_env(&store, "demo").expect("env should be created");
        fs::create_dir_all(store.root_bin_dir()).expect("root bin dir should exist");
        fs::write(
            store.root_bin_dir().join("root-tool"),
            "#!/bin/sh\necho root\n",
        )
        .expect("root binary should be written");
        fs::write(
            store.env_bin_dir("demo").join("env-tool"),
            "#!/bin/sh\necho env\n",
        )
        .expect("env binary should be written");

        set_env(&store, "demo").expect("env should be set");
        remove_env(&store, "demo").expect("env should be removed");

        let shim_dir = store.shim_bin_dir();
        assert!(shim_dir.join("root-tool").is_file());
        assert!(!shim_dir.join("env-tool").exists());

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn remove_package_deletes_binary_and_refreshes_shim() {
        let store = make_store();
        fs::create_dir_all(store.root_bin_dir()).expect("root bin dir should exist");
        fs::write(store.root_bin_dir().join("demo"), "#!/bin/sh\necho root\n")
            .expect("package binary should be written");
        super::env::sync_active_shims(&store).expect("shims should be created");

        let removed = remove_pkg(&store, "demo").expect("package should be removable");

        assert!(removed.removed);
        assert!(!store.root_bin_dir().join("demo").exists());
        assert!(!store.shim_bin_dir().join("demo").exists());

        fs::remove_dir_all(store.base).expect("failed to clean store dir");
    }

    #[test]
    fn build_toml_can_override_script_and_binary_location() {
        let repo_dir = make_temp_dir("repo");
        let root_bin_dir = make_temp_dir("root-bin");

        fs::create_dir_all(repo_dir.join("scripts")).expect("scripts dir should exist");
        fs::write(
            repo_dir.join("scripts").join("custom-build.sh"),
            "#!/bin/sh\nmkdir -p dist\nprintf '#!/bin/sh\\necho custom\\n' > dist/custom-bin\nchmod +x dist/custom-bin\n",
        )
        .expect("build script should be written");
        fs::write(
            repo_dir.join("build.toml"),
            "script = \"scripts/custom-build.sh\"\nbinary = \"dist/custom-bin\"\n",
        )
        .expect("build config should be written");

        let build_config =
            super::package::load_build_config(&repo_dir, "demo").expect("config should load");
        run_build_script(&repo_dir, &build_config).expect("custom build should succeed");
        let installed_binary =
            install_built_binary(&repo_dir, &build_config.binary, "demo", &root_bin_dir)
                .expect("custom binary should install");

        assert_eq!(installed_binary, root_bin_dir.join("demo"));
        let contents =
            fs::read_to_string(installed_binary).expect("failed to read installed binary");
        assert!(contents.contains("custom"));

        fs::remove_dir_all(repo_dir).expect("failed to clean repo dir");
        fs::remove_dir_all(root_bin_dir).expect("failed to clean root bin dir");
    }

    #[test]
    fn build_toml_can_override_installed_name() {
        let repo_dir = make_temp_dir("repo");
        let root_bin_dir = make_temp_dir("root-bin");

        fs::write(
            repo_dir.join("build"),
            "#!/bin/sh\nmkdir -p bin\nprintf '#!/bin/sh\\necho renamed\\n' > bin/original\nchmod +x bin/original\n",
        )
        .expect("build script should be written");
        fs::write(
            repo_dir.join("build.toml"),
            "name = \"renamed\"\nbinary = \"bin/original\"\n",
        )
        .expect("build config should be written");

        let build_config =
            super::package::load_build_config(&repo_dir, "demo").expect("config should load");
        run_build_script(&repo_dir, &build_config).expect("custom build should succeed");
        let installed_binary = install_built_binary(
            &repo_dir,
            &build_config.binary,
            &build_config.name,
            &root_bin_dir,
        )
        .expect("custom binary should install");

        assert_eq!(build_config.name, "renamed");
        assert_eq!(installed_binary, root_bin_dir.join("renamed"));

        fs::remove_dir_all(repo_dir).expect("failed to clean repo dir");
        fs::remove_dir_all(root_bin_dir).expect("failed to clean root bin dir");
    }
}
