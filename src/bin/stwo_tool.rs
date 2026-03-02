#![cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]

use std::path::PathBuf;

use anyhow::{Context, Result};
use arkade_compiler::stark_verify::{
    default_output_paths, execute_op_stark_verify, materialize_artifacts_from_proof,
    verify_stark_proof_from_files,
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "arkade-stwo")]
#[command(about = "Cairo1/STWO proof generation and verification utility", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Extract public inputs + default verification key JSON files from an existing STWO proof file.
    Materialize {
        #[arg(long)]
        proof: PathBuf,
        #[arg(long = "public-inputs")]
        public_inputs: Option<PathBuf>,
        #[arg(long = "verification-key")]
        verification_key: Option<PathBuf>,
    },
    /// Verify a STWO proof against public inputs and verification key JSON files.
    Verify {
        #[arg(long)]
        proof: PathBuf,
        #[arg(long = "public-inputs")]
        public_inputs: PathBuf,
        #[arg(long = "verification-key")]
        verification_key: PathBuf,
    },
    /// Execute the OP_STARK_VERIFY host binding directly (same checks as Verify).
    ExecuteOpcode {
        #[arg(long)]
        proof: PathBuf,
        #[arg(long = "public-inputs")]
        public_inputs: PathBuf,
        #[arg(long = "verification-key")]
        verification_key: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Materialize {
            proof,
            public_inputs,
            verification_key,
        } => {
            let output_dir = proof
                .parent()
                .context("proof path has no parent directory")?;
            let (default_public_inputs, default_verification_key) =
                default_output_paths(output_dir);

            let public_inputs_path = public_inputs.unwrap_or(default_public_inputs);
            let verification_key_path = verification_key.unwrap_or(default_verification_key);

            materialize_artifacts_from_proof(&proof, &public_inputs_path, &verification_key_path)?;

            println!(
                "Generated verification artifacts:\n- {}\n- {}",
                public_inputs_path.display(),
                verification_key_path.display()
            );
        }
        Commands::Verify {
            proof,
            public_inputs,
            verification_key,
        } => {
            verify_stark_proof_from_files(&proof, &public_inputs, &verification_key)?;
            println!("STWO proof verified successfully.");
        }
        Commands::ExecuteOpcode {
            proof,
            public_inputs,
            verification_key,
        } => {
            execute_op_stark_verify(&proof, &public_inputs, &verification_key)?;
            println!("OP_STARK_VERIFY execution succeeded.");
        }
    }

    Ok(())
}
