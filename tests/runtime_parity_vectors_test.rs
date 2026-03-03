mod parity_support;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

use parity_support::{load_vectors, materialize_bindings, to_stack_value};

#[test]
fn parity_vectors_match_expected_semantics() {
    let vectors = load_vectors();
    assert!(!vectors.is_empty(), "parity vectors are empty");

    let mut failures: Vec<String> = Vec::new();

    for vector in vectors {
        let mut env = ExecutionEnv::default();
        env.strict_placeholders = vector.strict_placeholders;
        env.bindings = materialize_bindings(&env, &vector.bindings);

        let mut vm = VMState::new(vector.asm.clone());
        let run = vm.run(&env);

        let observed_kind = match &run.outcome {
            VmOutcome::ScriptTrue => "script_true",
            VmOutcome::ScriptFalse => "script_false",
            VmOutcome::RuntimeError(_) => "runtime_error",
        };

        if observed_kind != vector.expected.kind {
            failures.push(format!(
                "{} kind mismatch: expected={}, got={}",
                vector.name, vector.expected.kind, observed_kind
            ));
            continue;
        }

        if let VmOutcome::RuntimeError(err) = &run.outcome {
            if let Some(expected_code) = &vector.expected.error_code {
                let actual = format!("{:?}", err.code);
                if actual != *expected_code {
                    failures.push(format!(
                        "{} error code mismatch: expected={}, got={}",
                        vector.name, expected_code, actual
                    ));
                }
            }
        }

        if let Some(expected_top) = &vector.expected.top {
            let expected = to_stack_value(&env, &env.bindings, expected_top);
            let actual = run.final_main_stack.last().cloned();
            if actual != Some(expected.clone()) {
                failures.push(format!(
                    "{} top mismatch: expected={:?}, got={:?}",
                    vector.name, expected, actual
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!("parity vector failures:\n{}", failures.join("\n"));
    }
}
