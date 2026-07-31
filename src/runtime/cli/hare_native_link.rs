use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use bun_bundler::options::{OutputFile, OutputFileValue, OutputKind};

const MANIFEST_VERSION: &str = "HARE-LINK-MANIFEST-1";

unsafe extern "C" {
    safe fn Bun__Hare__runNativeApplication(
        argc: i32,
        argv: *const *const core::ffi::c_char,
    ) -> i32;
}

pub(crate) struct LinkedTemplate {
    directory: PathBuf,
    ir: PathBuf,
    object: PathBuf,
    executable: PathBuf,
    executable_bytes: Box<[u8]>,
}

impl LinkedTemplate {
    pub(crate) fn executable_path(&self) -> &[u8] {
        &self.executable_bytes
    }
}

impl Drop for LinkedTemplate {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.ir);
        let _ = fs::remove_file(&self.object);
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.directory);
    }
}

struct LinkManifest {
    object_compiler: Box<str>,
    linker: Box<str>,
    prefix_response: Box<str>,
    suffix_response: Box<str>,
}

pub(crate) fn link_application(output_files: &[OutputFile]) -> Result<LinkedTemplate, Box<str>> {
    let mut native_ir = output_files
        .iter()
        .filter(|file| file.output_kind == OutputKind::HareLlvmIr);
    let file = native_ir
        .next()
        .ok_or_else(|| error("Hare bundle produced no native application IR"))?;
    if native_ir.next().is_some() {
        return Err(error(
            "Hare native convergence currently requires one bundled server entry chunk",
        ));
    }
    let OutputFileValue::Buffer { bytes: llvm_ir } = &file.value else {
        return Err(error("Hare native application IR is not memory-owned"));
    };

    let manifest = load_manifest()?;
    let directory = create_private_temp_directory()?;
    let ir = directory.join("application.ll");
    let object = directory.join(if cfg!(windows) {
        "application.obj"
    } else {
        "application.o"
    });
    let executable = directory.join(if cfg!(windows) {
        "hare-template.exe"
    } else {
        "hare-template"
    });
    let executable_bytes = path_bytes(&executable)?;
    let linked = LinkedTemplate {
        directory,
        ir,
        object,
        executable,
        executable_bytes,
    };

    let mut ir_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&linked.ir)
        .map_err(|cause| error(format!("failed to create Hare LLVM input: {cause}")))?;
    ir_file
        .write_all(llvm_ir)
        .map_err(|cause| error(format!("failed to write Hare LLVM input: {cause}")))?;
    drop(ir_file);

    let compiler = manifest.object_compiler.as_bytes();
    let ir_path = path_bytes(&linked.ir)?;
    let object_path = path_bytes(&linked.object)?;
    let compile_argv: [&[u8]; 8] = [
        compiler,
        b"-Wno-override-module",
        b"-x",
        b"ir",
        b"-c",
        &ir_path,
        b"-o",
        &object_path,
    ];
    let status = bun_core::spawn_sync_inherit_no_stdin(&compile_argv)
        .map_err(|cause| error(format!("failed to start Hare object compiler: {cause:?}")))?;
    if !status.is_ok() {
        return Err(error("Hare LLVM object compilation failed"));
    }

    let mut prefix_argument = Vec::with_capacity(manifest.prefix_response.len() + 1);
    prefix_argument.push(b'@');
    prefix_argument.extend_from_slice(manifest.prefix_response.as_bytes());
    let mut suffix_argument = Vec::with_capacity(manifest.suffix_response.len() + 1);
    suffix_argument.push(b'@');
    suffix_argument.extend_from_slice(manifest.suffix_response.as_bytes());
    let link_argv: [&[u8]; 7] = [
        manifest.linker.as_bytes(),
        &prefix_argument,
        &object_path,
        &suffix_argument,
        b"-o",
        linked.executable_path(),
        b"-Qunused-arguments",
    ];
    let status = bun_core::spawn_sync_inherit_no_stdin(&link_argv)
        .map_err(|cause| error(format!("failed to start Hare native linker: {cause:?}")))?;
    if !status.is_ok() {
        return Err(error("Hare native application link failed"));
    }
    if !linked.executable.is_file() {
        return Err(error("Hare native linker produced no executable"));
    }
    Ok(linked)
}

pub(crate) fn run_application() -> Option<i32> {
    let argv = bun_core::argv();
    let argc = i32::try_from(argv.len()).ok()?;
    let mut pointers: Vec<*const core::ffi::c_char> = argv
        .iter()
        .map(|argument| argument.as_ptr().cast())
        .collect();
    pointers.push(core::ptr::null());
    let status = Bun__Hare__runNativeApplication(argc, pointers.as_ptr());
    (status != i32::MIN).then_some(status)
}

fn load_manifest() -> Result<LinkManifest, Box<str>> {
    let executable = bun_core::self_exe_path().map_err(|cause| {
        error(format!(
            "failed to locate Hare compiler executable: {cause:?}"
        ))
    })?;
    let executable = path_from_bytes(executable.as_bytes())?;
    let directory = executable
        .parent()
        .ok_or_else(|| error("Hare compiler executable has no parent directory"))?;
    let manifest_path = directory.join("hare-link.manifest");
    let contents = fs::read_to_string(&manifest_path).map_err(|cause| {
        error(format!(
            "Hare native link manifest is unavailable at {}: {cause}",
            manifest_path.display()
        ))
    })?;
    let fields: Vec<&str> = contents.lines().collect();
    if fields.len() != 7 || fields[0] != MANIFEST_VERSION {
        return Err(error("Hare native link manifest has an unsupported schema"));
    }
    let expected_os = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        std::env::consts::OS
    };
    let expected_arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        std::env::consts::ARCH
    };
    if fields[1] != expected_os || fields[2] != expected_arch {
        return Err(error(format!(
            "Hare link manifest targets {}-{}, host is {expected_os}-{expected_arch}",
            fields[1], fields[2]
        )));
    }
    for response in [fields[5], fields[6]] {
        if !Path::new(response).is_file() {
            return Err(error(format!(
                "Hare native link response file is missing: {response}"
            )));
        }
    }
    Ok(LinkManifest {
        object_compiler: fields[3].into(),
        linker: fields[4].into(),
        prefix_response: fields[5].into(),
        suffix_response: fields[6].into(),
    })
}

fn create_private_temp_directory() -> Result<PathBuf, Box<str>> {
    let base = std::env::temp_dir();
    for _ in 0..16 {
        let path = base.join(format!(
            "bun-hare-{}-{:016x}",
            std::process::id(),
            bun_core::fast_random()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(
                        |cause| {
                            let _ = fs::remove_dir(&path);
                            error(format!(
                                "failed to secure Hare native link directory: {cause}"
                            ))
                        },
                    )?;
                }
                return Ok(path);
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(cause) => {
                return Err(error(format!(
                    "failed to create Hare native link directory: {cause}"
                )));
            }
        }
    }
    Err(error(
        "failed to allocate a unique Hare native link directory",
    ))
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Result<Box<[u8]>, Box<str>> {
    use std::os::unix::ffi::OsStrExt as _;
    Ok(path.as_os_str().as_bytes().into())
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, Box<str>> {
    use std::os::unix::ffi::OsStrExt as _;
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Result<Box<[u8]>, Box<str>> {
    path.to_str()
        .map(|value| value.as_bytes().into())
        .ok_or_else(|| error("Hare native link path is not UTF-8"))
}

#[cfg(windows)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, Box<str>> {
    let value = core::str::from_utf8(bytes)
        .map_err(|_| error("Hare compiler executable path is not UTF-8"))?;
    Ok(PathBuf::from(value))
}

fn error(message: impl Into<String>) -> Box<str> {
    message.into().into_boxed_str()
}
