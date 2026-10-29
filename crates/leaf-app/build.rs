//! Builds the app icon into `leaf.exe` on Windows, where Explorer, the taskbar and Alt+Tab read it from the
//! executable itself. GPUI loads icon resource 1 for its windows, which is what `leaf.rc` declares.
//! Other systems ignore this: macOS reads the icon from `Leaf.app`, Linux from the window and `.desktop` entry.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=resources/leaf.rc");
    println!("cargo:rerun-if-changed=resources/leaf.ico");
    // Required, not optional: a Windows build without its icon should fail rather than ship without one.
    embed_resource::compile("resources/leaf.rc", embed_resource::NONE).manifest_required()?;
    Ok(())
}
