//! Stage already-built Thor executables with their matching CEF framework.
//! This command never invokes a build or fetches a different browser runtime.

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use std::path::PathBuf;
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dist/cef"));
    if destination
        .join("Thor.app/Contents/Frameworks/Chromium.app")
        .exists()
    {
        anyhow::bail!("Refusing to package CEF over a legacy Chromium bundle");
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"))
        .join("release");
    for binary in ["Thor", "thor-labs"] {
        if !target.join(binary).is_file() {
            anyhow::bail!("Missing release executable: {binary}");
        }
    }
    let cef_directory = cef::sys::get_cef_dir()
        .ok_or_else(|| anyhow::anyhow!("Pinned CEF distribution unavailable"))?;
    if !cef_directory
        .join("Chromium Embedded Framework.framework")
        .is_dir()
    {
        anyhow::bail!("Pinned CEF framework is unavailable");
    }
    let info = cef::build_util::mac::BundleInfo::new(
        "Thor",
        "com.nirmalhk7.thor",
        "Thor",
        "en",
        env!("CARGO_PKG_VERSION").parse()?,
    );
    let app = cef::build_util::mac::bundle(&destination, &target, "Thor", "Thor", None, info)?;
    std::fs::copy(
        target.join("thor-labs"),
        app.join("Contents/MacOS/thor-labs"),
    )?;
    println!("{}", app.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Thor packaging currently targets macOS");
}
