use std::{env, fs, path::PathBuf};

struct ProgramIdSpec {
    env_key: &'static str,
    out_file: &'static str,
    default_id: &'static str,
}

const PROGRAM_ID_SPECS: &[ProgramIdSpec] = &[
    ProgramIdSpec {
        env_key: "PRIMITIVO_MERKLE_AIRDROP_ID",
        out_file: "primitivo_merkle_airdrop_program_id.rs",
        default_id: "Dpjs4ihZc6T9Y6mBfgDcmRavoFysLRDpdW5fezbxGZ33",
    },
    ProgramIdSpec {
        env_key: "PRIMITIVO_VESTING_ID",
        out_file: "vesting_program_id.rs",
        default_id: "8bSvkfYPuNqNRSSZzPD62H1dDPrACYPLLQitkYWVs75q",
    },
];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));

    for spec in PROGRAM_ID_SPECS {
        println!("cargo:rerun-if-env-changed={}", spec.env_key);
        generate_program_id_file(&out_dir, spec);
    }
}

fn generate_program_id_file(out_dir: &PathBuf, spec: &ProgramIdSpec) {
    let program_id = env::var(spec.env_key).unwrap_or_else(|_| spec.default_id.to_string());

    validate_program_id(spec.env_key, &program_id);

    let out_file = out_dir.join(spec.out_file);
    let contents = format!("anchor_lang::prelude::declare_id!(\"{}\");\n", program_id);
    fs::write(&out_file, contents).unwrap_or_else(|err| {
        panic!(
            "failed to write generated program id file {}: {err}",
            out_file.display()
        )
    });
}

fn validate_program_id(env_key: &str, value: &str) {
    const MIN_LEN: usize = 32;
    const MAX_LEN: usize = 44;
    const BASE58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    assert!(
        (MIN_LEN..=MAX_LEN).contains(&value.len()),
        "{env_key} must be {MIN_LEN}..={MAX_LEN} chars, got '{}'",
        value
    );
    assert!(
        value.chars().all(|c| BASE58.contains(c)),
        "{env_key} must be a base58 string, got '{}'",
        value
    );
}
