use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "dev".to_string());
    let ts = git(&["log", "-1", "--format=%ct"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    println!("cargo:rustc-env=YUME_GIT_HASH={hash}");
    println!("cargo:rustc-env=YUME_GIT_TS={ts}");
}
