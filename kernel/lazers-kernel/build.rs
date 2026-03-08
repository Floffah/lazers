use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=LAZERS_USER_ECHO_ELF");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let workspace_dir = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("kernel crate should live two directories under the workspace root")
        .to_path_buf();

    let user_elf = env::var("LAZERS_USER_ECHO_ELF").unwrap_or_else(|_| {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
        let placeholder = out_dir.join("placeholder-user-echo.elf");
        fs::write(&placeholder, []).expect("failed to write placeholder user elf");
        placeholder.to_string_lossy().into_owned()
    });
    let candidate = PathBuf::from(&user_elf);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        workspace_dir.join(candidate)
    };
    let user_elf = fs::canonicalize(&resolved)
        .unwrap_or(resolved)
        .to_string_lossy()
        .into_owned();

    println!("cargo:rustc-env=LAZERS_USER_ECHO_ELF={user_elf}");
}
