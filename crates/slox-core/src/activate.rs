use crate::store::StorePaths;
use slox_cli::ActivateCommand;

pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

fn nu_double_quote(value: &str) -> String {
    format!(
        r#""{}""#,
        value.replace('\\', r#"\\"#).replace('"', r#"\""#)
    )
}

pub(crate) fn activation_script(
    store: &StorePaths,
    cmd: ActivateCommand,
) -> Result<String, String> {
    let bin_path = store.shim_bin_dir().to_string_lossy().to_string();
    let store_path = store.base.to_string_lossy().to_string();

    match cmd {
        ActivateCommand::Sh | ActivateCommand::Bash | ActivateCommand::Zsh => Ok(format!(
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
            store = shell_single_quote(&store_path),
            bin = shell_single_quote(&bin_path)
        )),
        ActivateCommand::Nu => Ok(format!(
            concat!(
                "let slox_store = {store}\n",
                "let slox_bin = {bin}\n",
                "let slox_path_entries = (if (($env.PATH | describe) | str starts-with \"list\") {{\n",
                "  $env.PATH\n",
                "}} else {{\n",
                "  $env.PATH | split row (char esep)\n",
                "}})\n",
                "$env.PATH = (\n",
                "  $slox_path_entries\n",
                "  | each {{|entry| $entry | into string }}\n",
                "  | where {{|entry|\n",
                "      ($entry != $\"($slox_store)/bin\")\n",
                "      and ($entry != $\"($slox_store)/bin/\")\n",
                "    }}\n",
                "  | prepend $slox_bin\n",
                ")"
            ),
            store = nu_double_quote(&store_path),
            bin = nu_double_quote(&bin_path)
        )),
    }
}
