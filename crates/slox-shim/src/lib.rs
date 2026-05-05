use std::path::PathBuf;

pub struct Shim {
    target: PathBuf,
}

impl Shim {
    pub fn new(bin: &str) -> Self {
        let path = PathBuf::from(bin);
        if !path.exists() {
            panic!("Requested path is not available: {}", bin);
        }

        Self { target: path }
    }

    pub fn generate(&self) -> String {
        let target_path = self.target.to_string_lossy();

        // The shim targets a fixed absolute path so it remains stable regardless
        // of the working directory it is invoked from.
        // regardless of where it's called from.
        format!("#!/bin/sh\nexec \"{}\" \"$@\"", target_path)
    }
}
