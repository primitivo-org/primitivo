use std::{env, fs, path::PathBuf};

const ENV_KEY: &str = "PRIMITIVO_MERKLE_AIRDROP_ID";
const DEFAULT_ID: &str = "Dpjs4ihZc6T9Y6mBfgDcmRavoFysLRDpdW5fezbxGZ33";

fn main() {
    println!("cargo:rerun-if-env-changed={ENV_KEY}");

    let program_id = env::var(ENV_KEY).unwrap_or_else(|_| DEFAULT_ID.to_string());
    validate_program_id(ENV_KEY, &program_id);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let out_file = out_dir.join("merke_airdrop_program_id.rs");
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
