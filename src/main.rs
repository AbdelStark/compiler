use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
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
        /// Contract path (.json artifact or .ark source). If omitted, interactive picker is used.
        file: Option<String>,
        #[arg(long)]
        function: Option<String>,
        #[arg(long)]
        variant: Option<String>,
        #[arg(long = "bind")]
        bind: Vec<String>,
        #[arg(long, default_value_t = false)]
        trace: bool,
        #[arg(long, default_value_t = false)]
        strict: bool,
        /// Breakpoint instruction pointers for debugger start (repeatable)
        #[arg(long = "breakpoint")]
        breakpoint: Vec<usize>,
        /// Print discovered sample contract paths and exit.
        #[arg(long, default_value_t = false)]
        list_samples: bool,
        /// Force interactive source/function picker.
        #[arg(long, default_value_t = false)]
        pick: bool,
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
            breakpoint,
            list_samples,
            pick,
            headless,
        }) => debug_command(
            file,
            function,
            variant,
            bind,
            trace,
            strict,
            breakpoint,
            list_samples,
            pick,
            headless,
        ),
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

    let contract = load_contract_from_path(file)?;
    let program = runtime::load_program_from_contract(&contract, function, variant)?;
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
    file: Option<String>,
    function: Option<String>,
    variant: Option<String>,
    bind: Vec<String>,
    trace: bool,
    strict: bool,
    breakpoint: Vec<usize>,
    list_samples: bool,
    pick: bool,
    headless: bool,
) -> Result<i32> {
    init_tracing(trace);
    let samples = discover_sample_contracts()?;

    if list_samples {
        if samples.is_empty() {
            println!("No sample contracts found in examples/");
        } else {
            println!("Available sample contracts:");
            for sample in &samples {
                println!("- {}", sample);
            }
        }
        return Ok(0);
    }

    let interactive = pick || file.is_none() || function.is_none();

    let source_path = if interactive {
        pick_source_interactive(&samples)?
    } else {
        file.expect("checked above")
    };
    let contract = load_contract_from_path(&source_path)?;
    let (function, variant) = if interactive {
        pick_function_variant_interactive(&contract)?
    } else {
        let function = function.expect("checked above");
        let variant = match variant {
            Some(v) => parse_bool_arg(&v, "variant")?,
            None => false,
        };
        (function, variant)
    };

    let program = runtime::load_program_from_contract(&contract, &function, variant)?;
    let mut env = runtime::env::ExecutionEnv::default();
    env.strict_placeholders = strict;
    env.bindings = runtime::default_bindings_for_program(&program);
    env.bindings.extend(parse_bindings(&bind)?);

    println!(
        "Debug source: {}  Contract: {}  Function: {}  Variant: {}",
        source_path, program.contract_name, program.function_name, program.server_variant
    );

    let report = runtime::debugger::run_debugger(program.asm.clone(), &env, headless, breakpoint)?;

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
        if !report.breakpoints.is_empty() {
            println!(
                "Breakpoints: {}",
                report
                    .breakpoints
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
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

fn load_contract_from_path(path: &str) -> Result<models::ContractJson> {
    let file_path = Path::new(path);
    let ext = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "json" => {
            let raw = fs::read_to_string(path)
                .with_context(|| format!("failed reading artifact '{}'", file_path.display()))?;
            let contract = serde_json::from_str(&raw).with_context(|| {
                format!("failed parsing artifact json '{}'", file_path.display())
            })?;
            Ok(contract)
        }
        "ark" => {
            let source = fs::read_to_string(path)
                .with_context(|| format!("failed reading source '{}'", file_path.display()))?;
            let contract = compiler::compile(&source)
                .map_err(|err| anyhow::anyhow!("compilation error: {err}"))?;
            Ok(contract)
        }
        _ => anyhow::bail!(
            "unsupported contract extension for '{}'; expected .ark or .json",
            file_path.display()
        ),
    }
}

fn discover_sample_contracts() -> Result<Vec<String>> {
    let mut out = Vec::new();
    let dir = Path::new("examples");
    if !dir.exists() {
        return Ok(out);
    }

    for entry in fs::read_dir(dir).context("failed reading examples directory")? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let keep = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                let lower = ext.to_ascii_lowercase();
                lower == "ark" || lower == "json"
            })
            .unwrap_or(false);
        if !keep {
            continue;
        }

        out.push(path.display().to_string());
    }

    out.sort();
    Ok(out)
}

fn pick_source_interactive(samples: &[String]) -> Result<String> {
    println!("Interactive debug source selection");

    if samples.is_empty() {
        return prompt_non_empty(
            "No sample contracts found. Enter a custom path (.ark or .json): ",
        );
    }

    println!("Select a sample contract or enter a custom path:");
    println!("0) Enter custom path");
    for (idx, sample) in samples.iter().enumerate() {
        println!("{}) {}", idx + 1, sample);
    }

    loop {
        let raw = prompt_non_empty("Choice (number): ")?;
        let choice = raw
            .parse::<usize>()
            .with_context(|| format!("invalid choice '{raw}', expected a number"))?;
        if choice == 0 {
            return prompt_non_empty("Custom path (.ark or .json): ");
        }
        if (1..=samples.len()).contains(&choice) {
            return Ok(samples[choice - 1].clone());
        }
        println!("Choice out of range. Enter 0..{}.", samples.len());
    }
}

fn pick_function_variant_interactive(contract: &models::ContractJson) -> Result<(String, bool)> {
    if contract.functions.is_empty() {
        anyhow::bail!("contract '{}' has no functions", contract.name);
    }

    println!("Contract: {}", contract.name);
    println!("Select function variant:");
    for (idx, function) in contract.functions.iter().enumerate() {
        println!(
            "{}) {} (serverVariant={})",
            idx + 1,
            function.name,
            function.server_variant
        );
    }

    loop {
        let raw = prompt_non_empty("Function choice (number): ")?;
        let choice = raw
            .parse::<usize>()
            .with_context(|| format!("invalid choice '{raw}', expected a number"))?;
        if (1..=contract.functions.len()).contains(&choice) {
            let function = &contract.functions[choice - 1];
            return Ok((function.name.clone(), function.server_variant));
        }
        println!(
            "Choice out of range. Enter 1..{}.",
            contract.functions.len()
        );
    }
}

fn prompt_non_empty(prompt: &str) -> Result<String> {
    loop {
        print!("{prompt}");
        io::stdout().flush().context("failed flushing stdout")?;
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .context("failed reading stdin")?;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        println!("Input cannot be empty.");
    }
}
