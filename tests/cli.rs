mod common;
use common::Temp;
use std::process::Command;
fn call(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bastionkv"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn cli_natural_invocation_and_inspection() {
    let t = Temp::new();
    let p = t.db();
    let p = p.to_str().unwrap();
    for args in [
        vec!["init", p],
        vec!["put", p, "00ff", "0102", "--hex"],
        vec!["get", p, "00ff", "--hex"],
        vec!["scan", p, "00", "--hex"],
        vec!["stats", p],
        vec!["verify", p],
        vec!["dump-wal", p],
        vec!["compact", p],
    ] {
        let o = call(&args);
        assert!(
            o.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    assert_eq!(
        String::from_utf8(call(&["get", p, "00ff", "--hex"]).stdout).unwrap(),
        "0102\n"
    );
    assert!(call(&["dump-segment", t.segment().to_str().unwrap()])
        .status
        .success());
    assert!(call(&["delete", p, "00ff", "--hex"]).status.success());
    assert!(!call(&["get", p, "00ff", "--hex"]).status.success());
    assert!(!call(&["put", p, "0", "gg", "--hex"]).status.success());
    assert!(!call(&["wat", p]).status.success());
}
#[test]
fn cli_verify_corruption_nonzero_without_mutation() {
    let t = Temp::new();
    let p = t.db();
    let p = p.to_str().unwrap();
    assert!(call(&["init", p]).status.success());
    assert!(call(&["put", p, "x", "y"]).status.success());
    let wal = t.wal();
    let mut b = std::fs::read(&wal).unwrap();
    b[24] ^= 1;
    std::fs::write(&wal, &b).unwrap();
    let o = call(&["verify", p]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("offset"));
    assert_eq!(std::fs::read(wal).unwrap(), b);
}
