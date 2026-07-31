use hare_llvm::{
    AbiType, NativeBody, NativeFunction, NativeModule, TargetArchitecture, TargetLayout,
    TargetOperatingSystem,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn pinned_llvm_as() -> PathBuf {
    if let Some(path) = std::env::var_os("HARE_LLVM_AS") {
        return path.into();
    }

    let candidates = [
        "/usr/lib/llvm21/bin/llvm-as",
        "/usr/lib/llvm-21/bin/llvm-as",
        "/opt/homebrew/opt/llvm@21/bin/llvm-as",
        "/usr/local/opt/llvm@21/bin/llvm-as",
        "llvm-as",
    ];
    for candidate in candidates {
        let mut command = Command::new(candidate);
        command
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if command.status().is_ok_and(|status| status.success()) {
            return candidate.into();
        }
    }
    panic!("LLVM 21 assembler not found; set HARE_LLVM_AS to the pinned llvm-as executable");
}

#[test]
fn emits_deterministic_targeted_llvm() {
    let module = NativeModule {
        name: "hare-w1-probe".into(),
        target: TargetLayout::for_target(TargetOperatingSystem::Linux, TargetArchitecture::X86_64),
        functions: vec![NativeFunction {
            symbol: "Bun__Hare__emittedProbe".into(),
            parameters: vec![AbiType::I64, AbiType::I64],
            result: AbiType::I64,
            body: NativeBody::AddI64,
        }],
    };
    let first = module.emit_llvm_ir().unwrap();
    let second = module.emit_llvm_ir().unwrap();
    assert_eq!(first, second);
    assert!(first.contains("target triple = \"x86_64-unknown-linux-gnu\""));
    assert!(first.contains("define i64 @Bun__Hare__emittedProbe"));
}

#[test]
fn linked_native_probe_executes() {
    assert_eq!(hare_llvm::Bun__Hare__nativeProbe(0), 0x4841_5245_5f57_3101);
}

#[test]
fn pinned_llvm_assembles_emitted_module() {
    let module = NativeModule {
        name: "hare-assembler-probe".into(),
        target: TargetLayout::for_target(TargetOperatingSystem::Linux, TargetArchitecture::X86_64),
        functions: vec![NativeFunction {
            symbol: "Bun__Hare__assemblerProbe".into(),
            parameters: vec![AbiType::I64],
            result: AbiType::I64,
            body: NativeBody::XorI64(0x4841_5245),
        }],
    };
    let ir = module.emit_llvm_ir().unwrap();
    let llvm_as = pinned_llvm_as();

    let version = Command::new(&llvm_as).arg("--version").output().unwrap();
    let version_text = String::from_utf8_lossy(&version.stdout);
    assert!(
        version.status.success() && version_text.contains("LLVM version 21.1.8"),
        "Hare requires pinned LLVM 21.1.8, got: {version_text}"
    );

    let mut child = Command::new(&llvm_as)
        .args(["-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(ir.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "pinned llvm-as rejected Hare IR: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
