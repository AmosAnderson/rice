/// Linker module: invokes the system linker to produce a final executable.
///
/// Links the compiled object file with the Rice runtime static library.
/// On Unix: links librice.a via cc.
/// On Windows MSVC: links rice.lib via MSVC link.exe.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Library name varies by platform.
const RUNTIME_LIB_NAMES: &[&str] = if cfg!(target_os = "windows") {
    &["rice.lib", "librice.a"]
} else {
    &["librice.a"]
};

/// Find the rice static library.
/// Looks relative to the current executable's directory.
fn find_runtime_lib() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("finding exe path: {e}"))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "cannot find exe directory".to_string())?;

    let search_dirs = [
        exe_dir.to_path_buf(),
        exe_dir.join("deps"),
        // When running from target/debug/deps/ (tests), look up one level
        exe_dir.join(".."),
    ];

    for dir in &search_dirs {
        for name in RUNTIME_LIB_NAMES {
            let path = dir.join(name);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(format!(
        "cannot find Rice runtime library (searched for {:?} in {})",
        RUNTIME_LIB_NAMES,
        exe_dir.display()
    ))
}

/// Link an object file into a native executable.
pub fn link(object_bytes: &[u8], output_path: &Path) -> Result<(), String> {
    let obj_ext = if cfg!(target_os = "windows") {
        "obj"
    } else {
        "o"
    };
    let obj_path = output_path.with_extension(obj_ext);
    {
        let mut f = std::fs::File::create(&obj_path)
            .map_err(|e| format!("creating {}: {e}", obj_path.display()))?;
        f.write_all(object_bytes)
            .map_err(|e| format!("writing {}: {e}", obj_path.display()))?;
    }

    let runtime_lib = find_runtime_lib()?;

    let result = if cfg!(target_os = "windows") {
        link_windows(&obj_path, &runtime_lib, output_path)
    } else {
        link_unix(&obj_path, &runtime_lib, output_path)
    };

    // Clean up the object file
    let _ = std::fs::remove_file(&obj_path);

    result
}

// ── Windows MSVC linking ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn find_msvc_link() -> Result<PathBuf, String> {
    let vswhere = PathBuf::from(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );
    if !vswhere.exists() {
        return Err(
            "cannot find vswhere.exe — is Visual Studio or Build Tools installed?".to_string(),
        );
    }

    let output = Command::new(&vswhere)
        .args(["-latest", "-find", r"VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe"])
        .output()
        .map_err(|e| format!("running vswhere: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let link_path = stdout
        .lines()
        .next()
        .ok_or_else(|| "vswhere found no MSVC link.exe".to_string())?;

    let path = PathBuf::from(link_path.trim());
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("link.exe not found at {}", path.display()))
    }
}

#[cfg(target_os = "windows")]
fn find_msvc_lib_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // MSVC runtime libs (msvcrt, vcruntime, etc.)
    let vswhere = PathBuf::from(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );
    if vswhere.exists() {
        if let Ok(output) = Command::new(&vswhere)
            .args(["-latest", "-find", r"VC\Tools\MSVC\*\lib\x64"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let p = PathBuf::from(line.trim());
                if p.is_dir() {
                    paths.push(p);
                }
            }
        }
    }

    // Windows SDK: um and ucrt libs
    let sdk_root = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib");
    if sdk_root.is_dir() {
        // Find the latest SDK version
        if let Ok(entries) = std::fs::read_dir(&sdk_root) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            if let Some(latest) = versions.last() {
                let um = latest.join("um").join("x64");
                if um.is_dir() {
                    paths.push(um);
                }
                let ucrt = latest.join("ucrt").join("x64");
                if ucrt.is_dir() {
                    paths.push(ucrt);
                }
            }
        }
    }

    paths
}

#[cfg(target_os = "windows")]
fn link_windows(obj_path: &Path, runtime_lib: &Path, output_path: &Path) -> Result<(), String> {
    let link_exe = find_msvc_link()?;

    let output_with_ext = if output_path.extension().is_none() {
        output_path.with_extension("exe")
    } else {
        output_path.to_path_buf()
    };

    let mut cmd = Command::new(&link_exe);
    cmd.arg("/NOLOGO")
        .arg(format!("/OUT:{}", output_with_ext.display()))
        .arg(obj_path)
        .arg(runtime_lib);

    // Add library search paths so link.exe can find system libs
    for lib_path in find_msvc_lib_paths() {
        cmd.arg(format!("/LIBPATH:{}", lib_path.display()));
    }

    // Windows system libraries required by the Rust standard library and dependencies
    cmd.args([
        "kernel32.lib",
        "advapi32.lib",
        "bcrypt.lib",
        "ntdll.lib",
        "userenv.lib",
        "ws2_32.lib",
        // GUI/clipboard (needed by crossterm, rustyline/clipboard_win)
        "user32.lib",
        "gdi32.lib",
        // C runtime
        "msvcrt.lib",
        "legacy_stdio_definitions.lib",
    ]);

    let output = cmd
        .output()
        .map_err(|e| format!("running linker ({}): {e}", link_exe.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("linker failed:\n{stdout}{stderr}"));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn link_windows(_obj_path: &Path, _runtime_lib: &Path, _output_path: &Path) -> Result<(), String> {
    unreachable!()
}

// ── Unix linking (macOS, Linux) ───────────────────────────────────────

fn link_unix(obj_path: &Path, runtime_lib: &Path, output_path: &Path) -> Result<(), String> {
    let mut cmd = Command::new("cc");
    cmd.arg(obj_path)
        .arg(runtime_lib)
        .arg("-o")
        .arg(output_path);

    if cfg!(target_os = "macos") {
        cmd.arg("-framework").arg("Security");
        cmd.arg("-framework").arg("CoreFoundation");
        cmd.arg("-liconv");
        cmd.arg("-lresolv");
    }

    if cfg!(target_os = "linux") {
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("running linker (cc): {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("linker failed: {stderr}"));
    }

    Ok(())
}
