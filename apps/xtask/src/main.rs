//! Yume Vale xtask — build system tasks.
//!
//! Every subcommand is defined as a typed clap enum and produces a testable
//! `Vec<ExecStep>` plan.  The plan is then executed by the `execute` function.
//! Tests assert the plan is correct WITHOUT running external commands (RED→GREEN).

mod exec;
mod plans;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

use exec::execute_plan;
use plans::{InfraCommands, *};

#[derive(Parser, Debug)]
#[command(
    name = "xtask",
    about = "Yume Vale build system tasks",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        #[arg(long)]
        release: bool,
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    Test {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    Check,
    BuildWeb,
    DockerBuild {
        target: Option<String>,
    },
    Cert {
        #[arg(long)]
        force: bool,
    },
    Map {
        #[arg(long)]
        check: bool,
    },
    #[command(subcommand)]
    Infra(InfraCommands),
    ValidateWorld,
    ValidateAssets,
    PersistenceSmoke,
    Entropy,
    Status,
    WebServe {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long)]
        open: bool,
    },
    Server {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    Client {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    Tools {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (description, steps) = match cli.command {
        Commands::Build { release, args } => ("build", build_plan(release, &args)),
        Commands::Test { args } => ("test", test_plan(&args)),
        Commands::Check => ("check", check_plan()),
        Commands::BuildWeb => ("build-web", build_web_plan()),
        Commands::DockerBuild { target } => ("docker-build", docker_build_plan(target.as_deref())),
        Commands::Cert { force } => ("cert", cert_plan(force)),
        Commands::Map { check } => ("map", map_plan(check)),
        Commands::Infra(InfraCommands::Check) => ("infra check", infra_check_plan()),
        Commands::Infra(InfraCommands::Apply) => ("infra apply", infra_apply_plan()),
        Commands::ValidateWorld => ("validate-world", validate_world_plan()),
        Commands::ValidateAssets => ("validate-assets", validate_assets_plan()),
        Commands::PersistenceSmoke => ("persistence-smoke", persistence_smoke_plan()),
        Commands::Entropy => ("entropy", entropy_plan()),
        Commands::Status => ("status", status_plan()),
        Commands::WebServe { port, open } => ("web-serve", web_serve_plan(port, open)),
        Commands::Server { args } => ("server", run_server_plan(&args)),
        Commands::Client { args } => ("client", run_client_plan(&args)),
        Commands::Tools { args } => ("tools", tools_plan(&args)),
    };
    if steps.is_empty() {
        eprintln!("Nothing to do for '{description}'");
        return ExitCode::SUCCESS;
    }
    if execute_plan(&steps) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assert_step_cargo(s: &exec::ExecStep, expected_args: &[&str]) {
        assert_eq!(s.program, "cargo");
        let expected: Vec<String> = expected_args.iter().map(|a| a.to_string()).collect();
        assert_eq!(s.args, expected);
    }

    #[test]
    fn parse_build_default() {
        let cli = Cli::try_parse_from(["xtask", "build"]).unwrap();
        match cli.command {
            Commands::Build { release, args } => {
                assert!(!release);
                assert!(args.is_empty());
            }
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn parse_build_release() {
        let cli = Cli::try_parse_from(["xtask", "build", "--release"]).unwrap();
        match cli.command {
            Commands::Build { release, .. } => assert!(release),
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn parse_build_with_extra_args() {
        let cli = Cli::try_parse_from(["xtask", "build", "-p", "server"]).unwrap();
        match cli.command {
            Commands::Build { args, .. } => assert_eq!(args, vec!["-p", "server"]),
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn parse_test_default() {
        let cli = Cli::try_parse_from(["xtask", "test"]).unwrap();
        assert!(matches!(cli.command, Commands::Test { .. }));
    }

    #[test]
    fn parse_test_targeted() {
        let cli = Cli::try_parse_from(["xtask", "test", "-p", "game_core"]).unwrap();
        match cli.command {
            Commands::Test { args } => assert_eq!(args, vec!["-p", "game_core"]),
            _ => panic!("expected Test"),
        }
    }

    #[test]
    fn parse_check() {
        let cli = Cli::try_parse_from(["xtask", "check"]).unwrap();
        assert!(matches!(cli.command, Commands::Check));
    }

    #[test]
    fn parse_build_web() {
        let cli = Cli::try_parse_from(["xtask", "build-web"]).unwrap();
        assert!(matches!(cli.command, Commands::BuildWeb));
    }

    #[test]
    fn parse_docker_build() {
        let cli = Cli::try_parse_from(["xtask", "docker-build", "server"]).unwrap();
        match cli.command {
            Commands::DockerBuild { target } => assert_eq!(target.as_deref(), Some("server")),
            _ => panic!("expected DockerBuild"),
        }
    }

    #[test]
    fn parse_cert() {
        let cli = Cli::try_parse_from(["xtask", "cert"]).unwrap();
        assert!(matches!(cli.command, Commands::Cert { .. }));
    }

    #[test]
    fn parse_infra_check() {
        let cli = Cli::try_parse_from(["xtask", "infra", "check"]).unwrap();
        assert!(matches!(cli.command, Commands::Infra(InfraCommands::Check)));
    }

    #[test]
    fn parse_entropy() {
        let cli = Cli::try_parse_from(["xtask", "entropy"]).unwrap();
        assert!(matches!(cli.command, Commands::Entropy));
    }

    #[test]
    fn parse_server() {
        let cli = Cli::try_parse_from(["xtask", "server"]).unwrap();
        assert!(matches!(cli.command, Commands::Server { .. }));
    }

    #[test]
    fn parse_client() {
        let cli = Cli::try_parse_from(["xtask", "client"]).unwrap();
        assert!(matches!(cli.command, Commands::Client { .. }));
    }

    #[test]
    fn parse_tools() {
        let cli = Cli::try_parse_from(["xtask", "tools", "generate-cert"]).unwrap();
        match cli.command {
            Commands::Tools { args } => assert_eq!(args, vec!["generate-cert"]),
            _ => panic!("expected Tools"),
        }
    }

    // -----------------------------------------------------------------------
    // Plan correctness — build
    // -----------------------------------------------------------------------

    #[test]
    fn plan_build_default() {
        let steps = build_plan(false, &[]);
        assert_eq!(steps.len(), 1);
        assert_step_cargo(&steps[0], &["build", "--workspace"]);
    }

    #[test]
    fn plan_build_release() {
        let steps = build_plan(true, &[]);
        assert_step_cargo(&steps[0], &["build", "--release", "--workspace"]);
    }

    #[test]
    fn plan_build_with_p_flag_no_workspace() {
        let steps = build_plan(false, &["-p".to_string(), "server".to_string()]);
        assert_step_cargo(&steps[0], &["build", "-p", "server"]);
    }

    #[test]
    fn plan_build_with_explicit_workspace() {
        let steps = build_plan(false, &["--workspace".to_string()]);
        assert_step_cargo(&steps[0], &["build", "--workspace"]);
    }

    // -----------------------------------------------------------------------
    // Plan correctness — test
    // -----------------------------------------------------------------------

    #[test]
    fn plan_test_default() {
        let steps = test_plan(&[]);
        assert_eq!(steps[0].args, vec!["test", "--workspace"]);
    }

    #[test]
    fn plan_test_targeted() {
        let steps = test_plan(&["-p".to_string(), "game_core".to_string()]);
        assert_eq!(steps[0].args, vec!["test", "-p", "game_core"]);
    }

    #[test]
    fn plan_test_with_extra() {
        let steps = test_plan(&["--nocapture".to_string()]);
        assert!(steps[0].args.contains(&"--workspace".to_string()));
        assert!(steps[0].args.contains(&"--nocapture".to_string()));
    }

    // -----------------------------------------------------------------------
    // Plan correctness — check
    // -----------------------------------------------------------------------

    #[test]
    fn plan_check_three_steps() {
        let steps = check_plan();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].args[0], "fmt");
        assert_eq!(steps[2].args, vec!["test", "--workspace"]);
    }

    // -----------------------------------------------------------------------
    // Plan correctness — build-web
    // -----------------------------------------------------------------------

    #[test]
    fn plan_build_web_uses_trunk() {
        let steps = build_web_plan();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].program, "trunk");
        assert!(steps[0].with_wasm_toolchain);
    }

    // -----------------------------------------------------------------------
    // Plan correctness — docker
    // -----------------------------------------------------------------------

    #[test]
    fn plan_docker_build_server() {
        let steps = docker_build_plan(Some("server"));
        assert_eq!(steps.len(), 1);
        assert!(steps[0].args.contains(&"Dockerfile.server".to_string()));
    }

    #[test]
    fn plan_docker_build_client() {
        let steps = docker_build_plan(Some("client"));
        assert_eq!(steps.len(), 1);
        assert!(steps[0].args.contains(&"Dockerfile.client".to_string()));
    }

    #[test]
    fn plan_docker_build_all() {
        let steps = docker_build_plan(None);
        assert_eq!(steps.len(), 2);
    }

    #[test]
    #[should_panic(expected = "Unknown docker target")]
    fn plan_docker_build_unknown() {
        docker_build_plan(Some("worker"));
    }

    // -----------------------------------------------------------------------
    // Plan correctness — cert
    // -----------------------------------------------------------------------

    #[test]
    fn plan_cert_force_generates() {
        let steps = cert_plan(true);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].program, "cargo");
        assert!(steps[0].args.contains(&"generate-cert".to_string()));
    }

    // -----------------------------------------------------------------------
    // Plan correctness — map
    // -----------------------------------------------------------------------

    #[test]
    fn plan_map_check_no_serve() {
        let steps = map_plan(true);
        for s in &steps {
            assert!(!s.inherit_io, "map --check should not serve");
            assert_ne!(s.program, "python3", "map --check should not serve");
        }
    }

    #[test]
    fn plan_map_serve_uses_python3() {
        let steps = map_plan(false);
        assert!(steps.iter().any(|s| s.program == "python3"));
    }

    // -----------------------------------------------------------------------
    // Plan correctness — infra
    // -----------------------------------------------------------------------

    #[test]
    fn plan_infra_check_has_four_steps() {
        let steps = infra_check_plan();
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn plan_infra_apply_has_three_steps() {
        let steps = infra_apply_plan();
        assert_eq!(steps.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Plan correctness — server / client / tools
    // -----------------------------------------------------------------------

    #[test]
    fn plan_web_serve_defaults() {
        let steps = web_serve_plan(8080, false);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].program, "trunk");
        assert!(steps[0].inherit_io);
        assert!(steps[0].with_wasm_toolchain);
    }

    #[test]
    fn plan_server_inherits_io() {
        let steps = run_server_plan(&[]);
        assert!(steps[0].inherit_io);
    }

    #[test]
    fn plan_client_inherits_io() {
        let steps = run_client_plan(&[]);
        assert!(steps[0].inherit_io);
    }

    #[test]
    fn plan_tools_forward_args() {
        let steps = tools_plan(&["generate-cert".to_string()]);
        assert!(steps[0].args.contains(&"generate-cert".to_string()));
    }

    // -----------------------------------------------------------------------
    // Stub plans
    // -----------------------------------------------------------------------

    #[test]
    fn plan_validate_world_stub() {
        let steps = validate_world_plan();
        assert_eq!(steps[0].program, "echo");
    }

    #[test]
    fn plan_validate_assets_stub() {
        let steps = validate_assets_plan();
        assert_eq!(steps[0].program, "echo");
    }

    #[test]
    fn plan_entropy_stub() {
        let steps = entropy_plan();
        assert_eq!(steps[0].program, "echo");
    }

    #[test]
    fn plan_status_stub() {
        let steps = status_plan();
        assert_eq!(steps[0].program, "echo");
    }

    // -----------------------------------------------------------------------
    // ExecStep builder
    // -----------------------------------------------------------------------

    #[test]
    fn exec_step_builder() {
        use exec::ExecStep;
        let step = ExecStep::new("test step", "cargo")
            .args(&["build", "--release"])
            .work_dir(PathBuf::from("."))
            .ignore_failure()
            .with_wasm_toolchain()
            .inherit_io()
            .env("KEY", "VAL");
        assert_eq!(step.description, "test step");
        assert_eq!(step.args, vec!["build", "--release"]);
        assert!(step.ignore_failure);
        assert!(step.with_wasm_toolchain);
        assert!(step.inherit_io);
        assert_eq!(step.envs, vec![("KEY".to_string(), "VAL".to_string())]);
    }
}
