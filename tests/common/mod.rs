use std::process::Command;

pub fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ue-weave"))
}

pub fn directory() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ue-weave-test-")
        .tempdir()
        .unwrap()
}
