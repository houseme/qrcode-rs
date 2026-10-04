//! Competing CLI generation and validation inputs.

#![cfg(feature = "cli")]

use std::process::Command;

#[test]
fn generation_inputs_cannot_be_silently_ignored_by_the_validate_command() {
    for (arguments, expected) in [
        (vec!["alpha", "validate", "missing-image"], "TEXT cannot be used together with validate"),
        (
            vec!["--batch", "missing-records", "validate", "missing-image"],
            "--batch cannot be used together with validate",
        ),
        (
            vec!["alpha", "--batch", "missing-records", "validate", "missing-image"],
            "--batch cannot be used together with TEXT",
        ),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_qrencodes")).args(arguments).output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert_eq!(String::from_utf8(result.stderr).unwrap(), format!("error: {expected}\n"));
    }
}

#[test]
fn the_existing_batch_text_conflict_message_and_priority_are_preserved() {
    let result = Command::new(env!("CARGO_BIN_EXE_qrencodes"))
        .args(["alpha", "--batch", "missing-records", "--parallel", "-f", "png", "--size", "0"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(String::from_utf8(result.stderr).unwrap(), "error: --batch cannot be used together with TEXT\n");
}
