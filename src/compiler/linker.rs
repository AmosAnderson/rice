/// Linker module: invokes the system linker to produce a final executable.
///
/// Links the compiled .o file with the Rice runtime static library (librice.a).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Find the rice static library.
/// Looks relative to the current executable's directory.
fn find_runtime_lib() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("finding exe path: {e}"))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "cannot find exe directory".to_string())?;

    // Look for librice.a in likely places
    let candidates = [
        exe_dir.join("librice.a"),
        exe_dir.join("deps").join("librice.a"),
        // When running from target/debug/deps/ (tests), look up one level
        exe_dir.join("..").join("librice.a"),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(format!(
        "cannot find librice.a (searched in {})",
        exe_dir.display()
    ))
}

/// Link an object file into a native executable.
pub fn link(object_bytes: &[u8], output_path: &Path) -> Result<(), String> {
    // Write the object file to a temp location
    let obj_path = output_path.with_extension("o");
    {
        let mut f = std::fs::File::create(&obj_path)
            .map_err(|e| format!("creating {}: {e}", obj_path.display()))?;
        f.write_all(object_bytes)
            .map_err(|e| format!("writing {}: {e}", obj_path.display()))?;
    }

    let runtime_lib = find_runtime_lib()?;

    // Invoke cc to link
    let mut cmd = Command::new("cc");
    cmd.arg(&obj_path)
        .arg(&runtime_lib)
        .arg("-o")
        .arg(output_path);

    // On macOS, link against required system libraries
    if cfg!(target_os = "macos") {
        cmd.arg("-framework").arg("Security");
        cmd.arg("-framework").arg("CoreFoundation");
        cmd.arg("-liconv");
        cmd.arg("-lresolv");
    }

    // On Linux, link against common libraries
    if cfg!(target_os = "linux") {
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("running linker (cc): {e}"))?;

    // Clean up the .o file
    let _ = std::fs::remove_file(&obj_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("linker failed: {stderr}"));
    }

    Ok(())
}
