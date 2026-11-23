use mpaas_openapi_rs::{run, Config, Error};
use serde_json::json;
use std::{env, process::ExitCode};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        println!("mPaaS private-cloud mini-program client\n\nUsage: mpaas-openapi-rs <config.json>\n\nActions: uploadAndPublish, upload, getPackages, getResource, createTask,\n         getTaskDetail, getTaskList, changeTaskStatus\n\nReads one JSON configuration file. Results are JSON on stdout; logs use stderr.\nWrites are never retried automatically.");
        return ExitCode::SUCCESS;
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("mpaas-openapi-rs {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.len() != 1 {
        eprintln!("Usage: mpaas-openapi-rs <config.json> (or --help)");
        return ExitCode::from(2);
    }
    let config = match Config::from_file(&args[0]).await {
        Ok(config) => config,
        Err(error) => return print_error(error, None),
    };
    info!(action = %config.action, "starting mPaaS OpenAPI action");
    match run(&config).await {
        Ok(result) => {
            info!(action = %config.action, "mPaaS OpenAPI action completed");
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, Some(&config.action)),
    }
}

fn print_error(error: Error, action: Option<&str>) -> ExitCode {
    error!(
        kind = error.kind,
        phase = error.phase.as_deref().unwrap_or("configuration"),
        "mPaaS OpenAPI action failed"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": false,
            "action": action,
            "phase": error.phase,
            "packageId": error.package_id,
            "taskId": error.task_id,
            "taskStatus": error.task_status,
            "error": {"kind": error.kind, "message": error.message}
        }))
        .unwrap()
    );
    ExitCode::from(1)
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}
