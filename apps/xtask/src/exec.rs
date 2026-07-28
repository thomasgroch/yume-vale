use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A single step in a command plan.
#[derive(Debug, Clone)]
pub struct ExecStep {
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub work_dir: Option<PathBuf>,
    pub ignore_failure: bool,
    pub with_wasm_toolchain: bool,
    pub inherit_io: bool,
    pub envs: Vec<(String, String)>,
}

impl ExecStep {
    pub fn new(description: &str, program: &str) -> Self {
        Self {
            description: description.to_string(),
            program: program.to_string(),
            args: vec![],
            work_dir: None,
            ignore_failure: false,
            with_wasm_toolchain: false,
            inherit_io: false,
            envs: vec![],
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        for a in args {
            self.args.push(a.to_string());
        }
        self
    }

    pub fn work_dir(mut self, dir: PathBuf) -> Self {
        self.work_dir = Some(dir);
        self
    }

    pub fn ignore_failure(mut self) -> Self {
        self.ignore_failure = true;
        self
    }

    pub fn with_wasm_toolchain(mut self) -> Self {
        self.with_wasm_toolchain = true;
        self
    }

    pub fn inherit_io(mut self) -> Self {
        self.inherit_io = true;
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.envs.push((key.to_string(), value.to_string()));
        self
    }
}

/// Detect the wasm toolchain `bin/` directory.
pub fn wasm_toolchain_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("YUME_WASM_TOOLCHAIN_PATH") {
        let p = PathBuf::from(path);
        if p.join("cargo").exists() || p.join("cargo.exe").exists() {
            return Some(p);
        }
    }
    let output = Command::new("rustup")
        .args(["show", "home"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let home_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if home_str.is_empty() {
        return None;
    }
    let toolchains_dir = Path::new(&home_str).join("toolchains");
    let entries = std::fs::read_dir(&toolchains_dir).ok()?;
    for entry in entries.flatten() {
        let bin = entry.path().join("bin");
        if bin.join("cargo").exists() || bin.join("cargo.exe").exists() {
            return Some(bin);
        }
    }
    None
}

/// Build a `std::process::Command` for the given step.
pub fn build_command(step: &ExecStep) -> Command {
    let mut cmd = Command::new(&step.program);
    for a in &step.args {
        cmd.arg(a);
    }
    if let Some(ref dir) = step.work_dir {
        cmd.current_dir(dir);
    }
    if step.with_wasm_toolchain {
        if let Some(tc_bin) = wasm_toolchain_bin() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{}", tc_bin.display(), current_path);
            cmd.env("PATH", &new_path);
        } else {
            eprintln!("Warning: wasm toolchain not found, building with default PATH");
            eprintln!("  Set YUME_WASM_TOOLCHAIN_PATH to the toolchain bin/ directory");
        }
    }
    for (k, v) in &step.envs {
        cmd.env(k, v);
    }
    if step.inherit_io {
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
    }
    cmd
}

/// Run a single step and return true on success.
pub fn run_step(step: &ExecStep) -> bool {
    eprint!("==> {} ... ", step.description);
    let mut cmd = build_command(step);
    let status = cmd.status();
    match status {
        Ok(s) if s.success() => {
            eprintln!("OK");
            true
        }
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            if step.ignore_failure {
                eprintln!("FAILED (exit = {code}, ignored)");
                true
            } else {
                eprintln!("FAILED (exit = {code})");
                false
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            false
        }
    }
}

/// Execute an entire plan. Returns true if all steps succeeded.
pub fn execute_plan(steps: &[ExecStep]) -> bool {
    for step in steps {
        if !run_step(step) {
            return false;
        }
    }
    true
}

/// Return the user's home directory, cross-platform.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Returns the age of a file in seconds, or None on error.
pub fn cert_age_secs(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let elapsed = modified.elapsed().ok()?;
    Some(elapsed.as_secs())
}
