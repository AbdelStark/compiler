use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use arkade_compiler::{compiler, models, runtime};
use clap::{Parser as ClapParser, Subcommand};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

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
        #[arg(
            long,
            required_unless_present_any = ["matrix", "list_functions", "dump_default_context"]
        )]
        function: Option<String>,
        #[arg(long)]
        variant: Option<String>,
        /// Bindings in the form key=value (repeatable). value can be:
        /// - int (42)
        /// - bool (true/false)
        /// - bytes (hex:68656c6c6f)
        /// - symbol (raw string)
        #[arg(long = "bind")]
        bind: Vec<String>,
        /// JSON file containing bindings object
        #[arg(long = "bind-file")]
        bind_file: Option<String>,
        #[arg(long, default_value_t = false)]
        trace: bool,
        /// Strict placeholder resolution
        #[arg(long, default_value_t = false)]
        strict: bool,
        /// Strict runtime type coercion (no loose as_i64/as_bool conversions)
        #[arg(long, default_value_t = false)]
        strict_types: bool,
        /// Strict schema validation for bindings before execution
        #[arg(long, default_value_t = false)]
        strict_bindings: bool,
        /// Output format: plain | json | metrics
        #[arg(long, default_value = "plain")]
        output: String,
        /// Context fixture file path
        #[arg(long)]
        context_file: Option<String>,
        /// Context fixture as raw JSON string
        #[arg(long)]
        context_json: Option<String>,
        /// Reject unknown fields in context JSON
        #[arg(long, default_value_t = false)]
        context_strict: bool,
        /// Print available function variants and exit
        #[arg(long, default_value_t = false)]
        list_functions: bool,
        /// Print default synthetic context JSON and exit
        #[arg(long, default_value_t = false)]
        dump_default_context: bool,
        /// Include elapsed timings in output
        #[arg(long, default_value_t = false)]
        profile: bool,
        /// Execute all contract functions/variants in one run
        #[arg(long, default_value_t = false)]
        matrix: bool,
        /// Execution mode preset: development | simulation | ci | safety
        #[arg(long, default_value = "development")]
        mode: String,
        #[arg(long)]
        max_steps: Option<usize>,
        #[arg(long)]
        max_script_len: Option<usize>,
        #[arg(long)]
        max_main_stack_depth: Option<usize>,
        #[arg(long)]
        max_alt_stack_depth: Option<usize>,
        #[arg(long)]
        max_stack_growth_per_step: Option<usize>,
        #[arg(long)]
        max_opcode_budget: Option<usize>,
        #[arg(long = "allow-opcode")]
        allow_opcode: Vec<String>,
        #[arg(long)]
        seed: Option<u64>,
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

#[derive(Debug)]
struct RunCommandArgs {
    file: String,
    function: Option<String>,
    variant: Option<String>,
    bind: Vec<String>,
    bind_file: Option<String>,
    trace: bool,
    strict: bool,
    strict_types: bool,
    strict_bindings: bool,
    output: String,
    context_file: Option<String>,
    context_json: Option<String>,
    context_strict: bool,
    list_functions: bool,
    dump_default_context: bool,
    profile: bool,
    matrix: bool,
    mode: String,
    max_steps: Option<usize>,
    max_script_len: Option<usize>,
    max_main_stack_depth: Option<usize>,
    max_alt_stack_depth: Option<usize>,
    max_stack_growth_per_step: Option<usize>,
    max_opcode_budget: Option<usize>,
    allow_opcode: Vec<String>,
    seed: Option<u64>,
}

#[derive(Debug)]
struct DebugCommandArgs {
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
}

#[derive(Debug, Clone, Copy)]
enum RunOutputFormat {
    Plain,
    Json,
    Metrics,
}

#[derive(Debug, Clone)]
struct FunctionTarget {
    function_name: String,
    server_variant: bool,
}

#[derive(Debug, Serialize)]
struct FunctionExecutionReport {
    contract_name: String,
    function_name: String,
    server_variant: bool,
    outcome: String,
    error_code: Option<String>,
    error_message: Option<String>,
    final_main_stack: Vec<runtime::value::StackValue>,
    final_alt_stack: Vec<runtime::value::StackValue>,
    telemetry: Vec<runtime::telemetry::StepTelemetry>,
    trace_version: String,
    trace_id: String,
    seed: Option<u64>,
    elapsed_nanos: u64,
    policy_counters: runtime::telemetry::PolicyCounters,
    runtime_options: runtime::telemetry::RuntimeOptionsSnapshot,
}

impl FunctionExecutionReport {
    fn from_run(program: &runtime::LoadedProgram, run: runtime::vm::VmRunResult) -> Self {
        let (outcome, error_code, error_message) = match &run.outcome {
            runtime::vm::VmOutcome::ScriptTrue => ("script_true".to_string(), None, None),
            runtime::vm::VmOutcome::ScriptFalse => ("script_false".to_string(), None, None),
            runtime::vm::VmOutcome::RuntimeError(err) => (
                "runtime_error".to_string(),
                Some(format!("{:?}", err.code)),
                Some(err.to_string()),
            ),
        };

        Self {
            contract_name: program.contract_name.clone(),
            function_name: program.function_name.clone(),
            server_variant: program.server_variant,
            outcome,
            error_code,
            error_message,
            final_main_stack: run.final_main_stack,
            final_alt_stack: run.final_alt_stack,
            telemetry: run.telemetry,
            trace_version: run.trace_version,
            trace_id: run.trace_id,
            seed: run.seed,
            elapsed_nanos: run.elapsed_nanos,
            policy_counters: run.policy_counters,
            runtime_options: run.runtime_options,
        }
    }
}

#[derive(Debug, Serialize)]
struct RunExecutionReport {
    contract_name: String,
    matrix: bool,
    results: Vec<FunctionExecutionReport>,
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
            bind_file,
            trace,
            strict,
            strict_types,
            strict_bindings,
            output,
            context_file,
            context_json,
            context_strict,
            list_functions,
            dump_default_context,
            profile,
            matrix,
            mode,
            max_steps,
            max_script_len,
            max_main_stack_depth,
            max_alt_stack_depth,
            max_stack_growth_per_step,
            max_opcode_budget,
            allow_opcode,
            seed,
        }) => run_command(RunCommandArgs {
            file,
            function,
            variant,
            bind,
            bind_file,
            trace,
            strict,
            strict_types,
            strict_bindings,
            output,
            context_file,
            context_json,
            context_strict,
            list_functions,
            dump_default_context,
            profile,
            matrix,
            mode,
            max_steps,
            max_script_len,
            max_main_stack_depth,
            max_alt_stack_depth,
            max_stack_growth_per_step,
            max_opcode_budget,
            allow_opcode,
            seed,
        }),
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
        }) => debug_command(DebugCommandArgs {
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
        }),
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

fn run_command(args: RunCommandArgs) -> Result<i32> {
    init_tracing(args.trace);

    let output_format = parse_output_format(&args.output)?;
    let execution_mode = parse_execution_mode(&args.mode)?;

    if args.context_file.is_some() && args.context_json.is_some() {
        anyhow::bail!("--context-file and --context-json are mutually exclusive");
    }

    if args.dump_default_context {
        let json = runtime::env::TxContext::default().to_json_pretty()?;
        println!("{json}");
        return Ok(0);
    }

    let contract = load_contract_from_path(&args.file)?;

    if args.list_functions {
        if contract.functions.is_empty() {
            println!("No functions found in contract '{}'", contract.name);
            return Ok(0);
        }
        println!("Functions for contract '{}':", contract.name);
        for function in &contract.functions {
            println!(
                "- {} (serverVariant={})",
                function.name, function.server_variant
            );
        }
        return Ok(0);
    }

    let mut file_bindings = HashMap::new();
    if let Some(path) = args.bind_file.as_deref() {
        file_bindings = parse_bindings_file(path)?;
    }
    let cli_bindings = parse_bindings(&args.bind)?;

    let tx_context = load_tx_context(
        args.context_file.as_deref(),
        args.context_json.as_deref(),
        args.context_strict,
    )?
    .unwrap_or_default();

    let policy_overrides = runtime::env::RuntimePolicy {
        max_steps: args.max_steps,
        max_script_len: args.max_script_len,
        max_main_stack_depth: args.max_main_stack_depth,
        max_alt_stack_depth: args.max_alt_stack_depth,
        max_stack_growth_per_step: args.max_stack_growth_per_step,
        max_opcode_budget: args.max_opcode_budget,
        allowed_opcodes: None,
    };

    let allowed_opcodes = if args.allow_opcode.is_empty() {
        None
    } else {
        Some(args.allow_opcode.iter().cloned().collect::<BTreeSet<_>>())
    };

    let targets = if args.matrix {
        contract
            .functions
            .iter()
            .map(|func| FunctionTarget {
                function_name: func.name.clone(),
                server_variant: func.server_variant,
            })
            .collect::<Vec<_>>()
    } else {
        let function_name = args
            .function
            .clone()
            .context("--function is required unless --matrix is set")?;
        let server_variant = match args.variant.as_deref() {
            Some(raw) => parse_bool_arg(raw, "variant")?,
            None => false,
        };
        vec![FunctionTarget {
            function_name,
            server_variant,
        }]
    };

    if targets.is_empty() {
        anyhow::bail!("no function targets were selected for execution");
    }

    let mut results: Vec<FunctionExecutionReport> = Vec::new();

    for target in targets {
        let program = runtime::load_program_from_contract(
            &contract,
            &target.function_name,
            target.server_variant,
        )?;

        let mut env = runtime::env::ExecutionEnv::default().with_mode(execution_mode);
        env.strict_placeholders = args.strict;
        env.strict_types = args.strict_types;
        env.strict_bindings = args.strict_bindings;
        env.tx_context = tx_context.clone();
        env.seed = args.seed;
        env.runtime_policy = env.runtime_policy.with_overrides(&policy_overrides);
        env.runtime_policy.allowed_opcodes = allowed_opcodes.clone();

        env.bindings =
            runtime::default_bindings_for_program_with_context(&program, &env.tx_context);
        env.bindings.extend(file_bindings.clone());
        env.bindings.extend(cli_bindings.clone());

        let run = runtime::execute_program(&program, &env);
        results.push(FunctionExecutionReport::from_run(&program, run));
    }

    let report = RunExecutionReport {
        contract_name: contract.name,
        matrix: args.matrix,
        results,
    };

    let exit_code = summarize_exit_code(&report);
    match output_format {
        RunOutputFormat::Plain => render_plain_report(&report, args.profile),
        RunOutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        RunOutputFormat::Metrics => render_metrics_report(&report),
    }

    Ok(exit_code)
}

fn summarize_exit_code(report: &RunExecutionReport) -> i32 {
    let mut code = 0;
    for result in &report.results {
        match result.outcome.as_str() {
            "runtime_error" => return 2,
            "script_false" => code = 1,
            _ => {}
        }
    }
    code
}

fn render_plain_report(report: &RunExecutionReport, profile: bool) {
    if report.results.len() == 1 {
        let result = &report.results[0];
        println!(
            "Contract: {}  Function: {}  Variant: {}",
            result.contract_name, result.function_name, result.server_variant
        );
        match result.outcome.as_str() {
            "script_true" => println!("RESULT: true"),
            "script_false" => println!("RESULT: false"),
            _ => {
                println!("RESULT: runtime_error");
                if let Some(message) = &result.error_message {
                    println!("ERROR: {message}");
                }
            }
        }
        if profile {
            println!(
                "PROFILE: elapsed_nanos={} steps={} opcode_steps={} conditional_jumps={}",
                result.elapsed_nanos,
                result.policy_counters.steps,
                result.policy_counters.opcode_steps,
                result.policy_counters.conditional_jumps
            );
        }
        return;
    }

    println!(
        "Contract: {}  Matrix: {} function variants",
        report.contract_name,
        report.results.len()
    );
    for result in &report.results {
        let status = match result.outcome.as_str() {
            "script_true" => "true",
            "script_false" => "false",
            _ => "runtime_error",
        };
        println!(
            "- {} (serverVariant={}): {}",
            result.function_name, result.server_variant, status
        );
        if result.outcome == "runtime_error" {
            if let Some(message) = &result.error_message {
                println!("  error: {message}");
            }
        }
        if profile {
            println!(
                "  profile: elapsed_nanos={} steps={} opcode_steps={} conditional_jumps={}",
                result.elapsed_nanos,
                result.policy_counters.steps,
                result.policy_counters.opcode_steps,
                result.policy_counters.conditional_jumps
            );
        }
    }
}

fn render_metrics_report(report: &RunExecutionReport) {
    println!(
        "contract,function,server_variant,outcome,error_code,steps,opcode_steps,conditional_jumps,elapsed_nanos,trace_id"
    );
    for result in &report.results {
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            report.contract_name,
            result.function_name,
            result.server_variant,
            result.outcome,
            result.error_code.clone().unwrap_or_default(),
            result.policy_counters.steps,
            result.policy_counters.opcode_steps,
            result.policy_counters.conditional_jumps,
            result.elapsed_nanos,
            result.trace_id
        );
    }
}

fn load_tx_context(
    context_file: Option<&str>,
    context_json: Option<&str>,
    strict_unknown_fields: bool,
) -> Result<Option<runtime::env::TxContext>> {
    let raw = match (context_file, context_json) {
        (None, None) => return Ok(None),
        (Some(path), None) => fs::read_to_string(path)
            .with_context(|| format!("failed reading context file '{path}'"))?,
        (None, Some(json)) => json.to_string(),
        (Some(_), Some(_)) => {
            anyhow::bail!("--context-file and --context-json are mutually exclusive")
        }
    };

    let context = runtime::env::TxContext::from_json(&raw, strict_unknown_fields)?;
    Ok(Some(context))
}

fn parse_output_format(raw: &str) -> Result<RunOutputFormat> {
    match raw.to_ascii_lowercase().as_str() {
        "plain" => Ok(RunOutputFormat::Plain),
        "json" => Ok(RunOutputFormat::Json),
        "metrics" => Ok(RunOutputFormat::Metrics),
        _ => anyhow::bail!("invalid --output value '{raw}', expected plain|json|metrics"),
    }
}

fn parse_execution_mode(raw: &str) -> Result<runtime::env::ExecutionMode> {
    match raw.to_ascii_lowercase().as_str() {
        "development" => Ok(runtime::env::ExecutionMode::Development),
        "simulation" => Ok(runtime::env::ExecutionMode::Simulation),
        "ci" => Ok(runtime::env::ExecutionMode::Ci),
        "safety" => Ok(runtime::env::ExecutionMode::Safety),
        _ => {
            anyhow::bail!("invalid --mode value '{raw}', expected development|simulation|ci|safety")
        }
    }
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

fn parse_bindings_file(path: &str) -> Result<HashMap<String, runtime::value::StackValue>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed reading --bind-file path '{path}'"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid --bind-file json at '{path}'"))?;
    let obj = value
        .as_object()
        .context("--bind-file must contain a JSON object")?;

    let mut out = HashMap::new();
    for (key, value) in obj {
        let parsed = parse_binding_json_value(value)
            .with_context(|| format!("invalid binding value for key '{key}' in --bind-file"))?;
        out.insert(key.clone(), parsed);
    }

    Ok(out)
}

fn parse_binding_json_value(value: &serde_json::Value) -> Result<runtime::value::StackValue> {
    Ok(match value {
        serde_json::Value::Bool(v) => runtime::value::StackValue::Bool(*v),
        serde_json::Value::Number(v) => {
            let as_i64 = v
                .as_i64()
                .with_context(|| format!("number {v} is out of i64 range"))?;
            runtime::value::StackValue::Int(as_i64)
        }
        serde_json::Value::String(v) => parse_binding_value(v)?,
        serde_json::Value::Array(values) => {
            let mut bytes = Vec::with_capacity(values.len());
            for value in values {
                let Some(v) = value.as_u64() else {
                    anyhow::bail!("byte array must contain only unsigned integers");
                };
                if v > u8::MAX as u64 {
                    anyhow::bail!("byte value {v} is out of range 0..255");
                }
                bytes.push(v as u8);
            }
            runtime::value::StackValue::Bytes(bytes)
        }
        serde_json::Value::Object(map) => {
            let bind_type = map
                .get("type")
                .and_then(|v| v.as_str())
                .context("typed binding object must include string field 'type'")?;
            let bind_value = map
                .get("value")
                .context("typed binding object must include field 'value'")?;

            match bind_type {
                "int" => {
                    let value = bind_value
                        .as_i64()
                        .context("typed int binding requires i64 value")?;
                    runtime::value::StackValue::Int(value)
                }
                "bool" => {
                    let value = bind_value
                        .as_bool()
                        .context("typed bool binding requires boolean value")?;
                    runtime::value::StackValue::Bool(value)
                }
                "bytes_hex" => {
                    let value = bind_value
                        .as_str()
                        .context("typed bytes_hex binding requires string value")?;
                    let bytes = hex::decode(value)
                        .with_context(|| format!("invalid bytes_hex payload '{value}'"))?;
                    runtime::value::StackValue::Bytes(bytes)
                }
                "bytes_utf8" => {
                    let value = bind_value
                        .as_str()
                        .context("typed bytes_utf8 binding requires string value")?;
                    runtime::value::StackValue::Bytes(value.as_bytes().to_vec())
                }
                "symbol" => {
                    let value = bind_value
                        .as_str()
                        .context("typed symbol binding requires string value")?;
                    runtime::value::StackValue::Symbol(value.to_string())
                }
                other => anyhow::bail!(
                    "unsupported typed binding '{other}', expected int|bool|bytes_hex|bytes_utf8|symbol"
                ),
            }
        }
        _ => anyhow::bail!("unsupported binding JSON value shape"),
    })
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

fn debug_command(args: DebugCommandArgs) -> Result<i32> {
    init_tracing(args.trace);
    let samples = discover_sample_contracts()?;

    if args.list_samples {
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

    let interactive = args.pick || args.file.is_none() || args.function.is_none();

    let source_path = if interactive {
        pick_source_interactive(&samples)?
    } else {
        args.file.expect("checked above")
    };
    let contract = load_contract_from_path(&source_path)?;
    let (function, variant) = if interactive {
        pick_function_variant_interactive(&contract)?
    } else {
        let function = args.function.expect("checked above");
        let variant = match args.variant {
            Some(v) => parse_bool_arg(&v, "variant")?,
            None => false,
        };
        (function, variant)
    };

    let program = runtime::load_program_from_contract(&contract, &function, variant)?;
    let mut env = runtime::env::ExecutionEnv {
        strict_placeholders: args.strict,
        bindings: runtime::default_bindings_for_program(&program),
        ..runtime::env::ExecutionEnv::default()
    };
    env.bindings.extend(parse_bindings(&args.bind)?);

    println!(
        "Debug source: {}  Contract: {}  Function: {}  Variant: {}",
        source_path, program.contract_name, program.function_name, program.server_variant
    );

    let report =
        runtime::debugger::run_debugger(program.asm.clone(), &env, args.headless, args.breakpoint)?;

    if args.headless {
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

fn init_tracing(trace: bool) {
    let filter = if trace {
        EnvFilter::new("arkade_runtime=debug")
    } else {
        EnvFilter::new("arkade_runtime=info")
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
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
