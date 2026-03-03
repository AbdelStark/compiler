use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use clap::{Parser as ClapParser, Subcommand};
use tracing_subscriber::EnvFilter;

mod compiler;
mod models;
mod opcodes;
mod parser;
mod runtime;

/// Arkade Compiler CLI
///
/// This is the command-line interface for the Arkade Compiler.
/// It compiles Arkade Script source code (.ark files) into JSON output
/// that represents Bitcoin Taproot scripts.
///
/// The JSON output includes:
/// - Contract name
/// - Parameters
/// - Server key placeholder
/// - Script paths for each function
///
/// Each script path includes a serverVariant flag. When using the script:
/// - If serverVariant is true, use the script as-is
/// - If serverVariant is false, libraries should add an exit delay timelock
///   (default 48 hours) for additional security

#[derive(ClapParser, Debug)]
#[command(name = "arkadec")]
#[command(about = "Arkade Compiler for Bitcoin Taproot scripts", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Legacy positional compile mode: `arkadec contract.ark -o out.json`
    #[arg(required = false)]
    file: Option<String>,

    /// Legacy compile output path
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Compile Arkade source (.ark) into artifact JSON
    Compile {
        file: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Execute artifact JSON in-process
    Run {
        file: String,
        #[arg(long)]
        function: String,
        #[arg(long, default_value = "false")]
        variant: String,
        /// Bindings in the form key=value (repeatable). value can be:
        /// - int (42)
        /// - bool (true/false)
        /// - bytes (hex:68656c6c6f)
        /// - symbol (raw string)
        #[arg(long = "bind")]
        bind: Vec<String>,
        #[arg(long, default_value_t = false)]
        trace: bool,
        #[arg(long, default_value_t = false)]
        strict: bool,
    },
    /// Launch TUI debugger for artifact JSON
    Debug {
        file: String,
        #[arg(long)]
        function: String,
        #[arg(long, default_value = "false")]
        variant: String,
        #[arg(long = "bind")]
        bind: Vec<String>,
        #[arg(long, default_value_t = false)]
        trace: bool,
        #[arg(long, default_value_t = false)]
        strict: bool,
        /// Initialize debugger panes and controls without interactive loop
        #[arg(long, default_value_t = false)]
        headless: bool,
    },
}

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("runtime_error: {err}");
            2
        }
    };
    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    let args = Args::parse();

    match args.command {
        Some(Command::Compile { file, output }) => compile_command(&file, output),
        Some(Command::Run {
            file,
            function,
            variant,
            bind,
            trace,
            strict,
        }) => run_command(&file, &function, variant, bind, trace, strict),
        Some(Command::Debug {
            file,
            function,
            variant,
            bind,
            trace,
            strict,
            headless,
        }) => debug_command(&file, &function, variant, bind, trace, strict, headless),
        None => {
            let file = args
                .file
                .as_deref()
                .context("either a subcommand or legacy <file.ark> argument is required")?;
            compile_command(file, args.output)
        }
    }
}

fn compile_command(file: &str, output: Option<String>) -> Result<i32> {
    let file_path = Path::new(file);
    if file_path.extension().unwrap_or_default() != "ark" {
        anyhow::bail!("input file must have .ark extension");
    }

    let source_code = fs::read_to_string(file)
        .with_context(|| format!("failed reading source file '{}'", file_path.display()))?;

    let output_json = compiler::compile(&source_code)
        .map_err(|err| anyhow::anyhow!("compilation error: {}", err))?;

    let output_path = match output {
        Some(path) => path,
        None => {
            let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();
            format!("{}.json", stem)
        }
    };

    let json = serde_json::to_string_pretty(&output_json)?;
    fs::write(&output_path, json)
        .with_context(|| format!("failed writing output file '{output_path}'"))?;

    println!("Compilation successful. Output written to {}", output_path);
    Ok(0)
}

fn run_command(
    file: &str,
    function: &str,
    variant: String,
    bind: Vec<String>,
    trace: bool,
    strict: bool,
) -> Result<i32> {
    init_tracing(trace);
    let variant = parse_bool_arg(&variant, "variant")?;

    let program = runtime::load_program_from_file(file, function, variant)?;
    let mut env = runtime::env::ExecutionEnv::default();
    env.strict_placeholders = strict;
    env.bindings = runtime::default_bindings_for_program(&program);
    env.bindings.extend(parse_bindings(&bind)?);

    let result = runtime::execute_program(&program, &env);
    println!(
        "Contract: {}  Function: {}  Variant: {}",
        program.contract_name, program.function_name, program.server_variant
    );

    match result.outcome {
        runtime::vm::VmOutcome::ScriptTrue => {
            println!("RESULT: true");
            Ok(0)
        }
        runtime::vm::VmOutcome::ScriptFalse => {
            println!("RESULT: false");
            Ok(1)
        }
        runtime::vm::VmOutcome::RuntimeError(err) => {
            println!("RESULT: runtime_error");
            println!("ERROR: {err}");
            Ok(2)
        }
    }
}

fn debug_command(
    file: &str,
    function: &str,
    variant: String,
    bind: Vec<String>,
    trace: bool,
    strict: bool,
    headless: bool,
) -> Result<i32> {
    init_tracing(trace);
    let variant = parse_bool_arg(&variant, "variant")?;

    let program = runtime::load_program_from_file(file, function, variant)?;
    let mut env = runtime::env::ExecutionEnv::default();
    env.strict_placeholders = strict;
    env.bindings = runtime::default_bindings_for_program(&program);
    env.bindings.extend(parse_bindings(&bind)?);

    let report = runtime::debugger::run_debugger(program.asm.clone(), &env, headless)?;

    if headless {
        println!("Debugger initialized");
        println!("Panes:");
        for pane in &report.panes {
            println!("- {}", pane);
        }
        println!("Controls:");
        for control in &report.controls {
            println!("- {}", control);
        }
    }

    Ok(0)
}

fn parse_bindings(bind: &[String]) -> Result<HashMap<String, runtime::value::StackValue>> {
    let mut out = HashMap::new();
    for pair in bind {
        let (key, value_raw) = pair
            .split_once('=')
            .with_context(|| format!("invalid --bind format '{}', expected key=value", pair))?;
        let value = parse_binding_value(value_raw)?;
        out.insert(key.to_string(), value);
    }
    Ok(out)
}

fn parse_binding_value(raw: &str) -> Result<runtime::value::StackValue> {
    if raw.eq_ignore_ascii_case("true") {
        return Ok(runtime::value::StackValue::Bool(true));
    }
    if raw.eq_ignore_ascii_case("false") {
        return Ok(runtime::value::StackValue::Bool(false));
    }
    if let Ok(v) = raw.parse::<i64>() {
        return Ok(runtime::value::StackValue::Int(v));
    }
    if let Some(hex_raw) = raw.strip_prefix("hex:") {
        let bytes = hex::decode(hex_raw)
            .with_context(|| format!("invalid hex payload in binding value '{raw}'"))?;
        return Ok(runtime::value::StackValue::Bytes(bytes));
    }
    Ok(runtime::value::StackValue::Symbol(raw.to_string()))
}

fn init_tracing(trace: bool) {
    let filter = if trace {
        EnvFilter::new("arkade_runtime=debug")
    } else {
        EnvFilter::new("arkade_runtime=info")
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn parse_bool_arg(raw: &str, arg_name: &str) -> Result<bool> {
    raw.parse::<bool>()
        .with_context(|| format!("invalid --{arg_name} value '{raw}', expected true or false"))
}
