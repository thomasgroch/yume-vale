use std::path::{Path, PathBuf};

use crate::exec::{ExecStep, cert_age_secs, home_dir};

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum InfraCommands {
    Check,
    Apply,
}

pub fn build_plan(release: bool, extra_args: &[String]) -> Vec<ExecStep> {
    let mut args = vec!["build".to_string()];
    if release {
        args.push("--release".to_string());
    }
    if !extra_args
        .iter()
        .any(|a| a == "--workspace" || a.starts_with("-p"))
    {
        args.push("--workspace".to_string());
    }
    args.extend(extra_args.iter().cloned());
    vec![
        ExecStep::new("Building workspace", "cargo")
            .args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
    ]
}

pub fn test_plan(extra_args: &[String]) -> Vec<ExecStep> {
    let mut args = vec!["test".to_string()];
    if !extra_args
        .iter()
        .any(|a| a == "--workspace" || a.starts_with("-p"))
    {
        args.push("--workspace".to_string());
    }
    args.extend(extra_args.iter().cloned());
    vec![
        ExecStep::new("Running tests", "cargo")
            .args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
    ]
}

pub fn check_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Checking formatting", "cargo").args(&["fmt", "--all", "--", "--check"]),
        ExecStep::new("Running clippy", "cargo").args(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
        ExecStep::new("Running tests", "cargo").args(&["test", "--workspace"]),
    ]
}

pub fn build_web_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Building wasm client", "trunk")
            .args(&["build"])
            .work_dir(PathBuf::from("apps/client"))
            .with_wasm_toolchain(),
    ]
}

pub fn docker_build_plan(target: Option<&str>) -> Vec<ExecStep> {
    match target {
        Some("server") => vec![
            ExecStep::new("Building server Docker image", "docker").args(&[
                "build",
                "-f",
                "Dockerfile.server",
                "-t",
                "yume-vale-server",
                ".",
            ]),
        ],
        Some("client") => vec![
            ExecStep::new("Building client Docker image", "docker").args(&[
                "build",
                "-f",
                "Dockerfile.client",
                "-t",
                "yume-vale-client",
                ".",
            ]),
        ],
        Some(other) => panic!("Unknown docker target: {other}. Use 'server' or 'client'."),
        None => vec![
            ExecStep::new("Building server Docker image", "docker").args(&[
                "build",
                "-f",
                "Dockerfile.server",
                "-t",
                "yume-vale-server",
                ".",
            ]),
            ExecStep::new("Building client Docker image", "docker").args(&[
                "build",
                "-f",
                "Dockerfile.client",
                "-t",
                "yume-vale-client",
                ".",
            ]),
        ],
    }
}

pub fn cert_plan(force: bool) -> Vec<ExecStep> {
    let cert_path = Path::new("certs/server.pem");
    let needs_gen = force
        || !cert_path.exists()
        || cert_age_secs(cert_path)
            .map(|s| s > 7 * 86400)
            .unwrap_or(true);
    if needs_gen {
        vec![
            ExecStep::new("Generating WebTransport dev certificate", "cargo").args(&[
                "run",
                "-p",
                "tools",
                "--",
                "generate-cert",
            ]),
        ]
    } else {
        let msg = "Certificate is fresh (less than 7 days old), skipping";
        vec![ExecStep::new(msg, "echo").args(&[msg])]
    }
}

fn infra_ansible_collections_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("yume-vale")
        .join("ansible")
        .join("collections")
}

fn infra_ansible_work_dir() -> PathBuf {
    PathBuf::from("infra/ansible")
}

fn infra_install_collections_step(work_dir: &Path, collections_dir: &Path) -> ExecStep {
    ExecStep::new("Installing Ansible collections", "ansible-galaxy")
        .args(&[
            "collection",
            "install",
            "-r",
            "requirements.yml",
            "-p",
            collections_dir.to_str().unwrap(),
            "--upgrade",
        ])
        .work_dir(work_dir.to_path_buf())
}

fn infra_playbook_step(work_dir: &Path, collections_dir: &Path, check_mode: bool) -> ExecStep {
    let mut args: Vec<&str> = Vec::new();
    if check_mode {
        args.push("--check");
        args.push("--diff");
    }
    args.push("playbooks/yume-firewall.yml");
    let mut step = ExecStep::new(
        if check_mode {
            "Checking firewall rules (dry-run)"
        } else {
            "Applying firewall rules"
        },
        "ansible-playbook",
    )
    .args(&args)
    .work_dir(work_dir.to_path_buf())
    .env(
        "ANSIBLE_COLLECTIONS_PATH",
        collections_dir.to_str().unwrap(),
    );
    if let Ok(key_path) = std::env::var("YUME_SSH_KEY") {
        step = step.env("ANSIBLE_PRIVATE_KEY_FILE", &key_path);
    }
    step
}

pub fn infra_check_plan() -> Vec<ExecStep> {
    let work_dir = infra_ansible_work_dir();
    let collections_dir = infra_ansible_collections_dir();
    vec![
        ExecStep::new("Ensuring Ansible collections cache directory", "mkdir")
            .args(&["-p"])
            .arg(collections_dir.to_str().unwrap()),
        infra_install_collections_step(&work_dir, &collections_dir),
        infra_playbook_step(&work_dir, &collections_dir, true),
        ExecStep::new("Validating K8s manifests (schema + dry-run)", "kubectl")
            .args(&[
                "apply",
                "--server-side",
                "--dry-run=server",
                "-f",
                "deploy/",
                "--validate=strict",
            ])
            .ignore_failure(),
    ]
}

pub fn infra_apply_plan() -> Vec<ExecStep> {
    let work_dir = infra_ansible_work_dir();
    let collections_dir = infra_ansible_collections_dir();
    vec![
        ExecStep::new("Ensuring Ansible collections cache directory", "mkdir")
            .args(&["-p"])
            .arg(collections_dir.to_str().unwrap()),
        infra_install_collections_step(&work_dir, &collections_dir),
        infra_playbook_step(&work_dir, &collections_dir, false),
    ]
}

pub fn map_plan(check_only: bool) -> Vec<ExecStep> {
    let cache = home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("yume-vale")
        .join("grasp");
    let needs_download = {
        let index = cache.join("index.html");
        !index.exists() || cert_age_secs(&index).map(|s| s > 7 * 86400).unwrap_or(true)
    };
    let mut steps = Vec::new();
    if needs_download {
        steps.push(
            ExecStep::new("Creating cache directory", "mkdir")
                .args(&["-p"])
                .arg(cache.to_str().unwrap()),
        );
        steps.push(
            ExecStep::new("Downloading Grasp viewer", "curl")
                .args(&[
                    "-fsSL",
                    "https://raw.githubusercontent.com/ashfordeOU/grasp/main/index.html",
                    "-o",
                ])
                .arg(cache.join("index.html").to_str().unwrap()),
        );
    }
    if !check_only {
        steps.push(
            ExecStep::new("Serving Grasp viewer (Ctrl+C to stop)", "python3")
                .args(&["-m", "http.server", "8765"])
                .work_dir(cache)
                .inherit_io(),
        );
    }
    steps
}

pub fn validate_world_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Validating world assets and config", "echo")
            .args(&["World validation — not yet implemented"]),
    ]
}

pub fn validate_assets_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Validating GLB assets exist", "echo")
            .args(&["Assets validation — not yet implemented"]),
    ]
}

pub fn persistence_smoke_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Running persistence smoke tests", "echo")
            .args(&["Persistence smoke tests — not yet implemented"]),
    ]
}

pub fn entropy_plan() -> Vec<ExecStep> {
    vec![
        ExecStep::new("Checking code entropy", "echo")
            .args(&["Entropy check — not yet implemented"]),
    ]
}

pub fn status_plan() -> Vec<ExecStep> {
    vec![ExecStep::new("Showing deployment status", "echo").args(&["Status — not yet implemented"])]
}

pub fn web_serve_plan(port: u16, open: bool) -> Vec<ExecStep> {
    let port_str = port.to_string();
    let mut trunk_args: Vec<&str> = vec!["serve", "--address", "127.0.0.1", "--port", &port_str];
    if open {
        trunk_args.push("--open");
    }
    vec![
        ExecStep::new("Serving wasm client", "trunk")
            .args(&trunk_args)
            .work_dir(PathBuf::from("apps/client"))
            .with_wasm_toolchain()
            .inherit_io(),
    ]
}

pub fn run_server_plan(extra_args: &[String]) -> Vec<ExecStep> {
    let mut cmd_args = vec!["run", "-p", "server", "--"];
    cmd_args.extend(extra_args.iter().map(|s| s.as_str()));
    vec![
        ExecStep::new("Starting server", "cargo")
            .args(&cmd_args)
            .inherit_io(),
    ]
}

pub fn run_client_plan(extra_args: &[String]) -> Vec<ExecStep> {
    let mut cmd_args = vec!["run", "-p", "client", "--"];
    cmd_args.extend(extra_args.iter().map(|s| s.as_str()));
    vec![
        ExecStep::new("Starting client", "cargo")
            .args(&cmd_args)
            .inherit_io(),
    ]
}

pub fn tools_plan(extra_args: &[String]) -> Vec<ExecStep> {
    let mut cmd_args = vec!["run", "-p", "tools", "--"];
    cmd_args.extend(extra_args.iter().map(|s| s.as_str()));
    vec![ExecStep::new("Running tools", "cargo").args(&cmd_args)]
}
