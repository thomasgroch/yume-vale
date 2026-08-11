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
    let full_hash = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| hash.clone());
    let ts = git(&["log", "-1", "--format=%ct"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    // In CI (detached HEAD after checkout) this falls back to "HEAD" — the
    // deploy pipeline builds from a plain checkout of `main`, not a branch
    // ref, so there's no better answer available at build time.
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "HEAD".to_string());
    println!("cargo:rustc-env=YUME_GIT_HASH={hash}");
    println!("cargo:rustc-env=YUME_GIT_FULL_HASH={full_hash}");
    println!("cargo:rustc-env=YUME_GIT_TS={ts}");
    println!("cargo:rustc-env=YUME_GIT_BRANCH={branch}");
}
