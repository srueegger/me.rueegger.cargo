use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=data/cargo-app.gresource.xml");
    println!("cargo:rerun-if-changed=data/ui/window.ui");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target = format!("{}/cargo-app.gresource", out_dir);

    let output = Command::new("glib-compile-resources")
        .args([
            "--sourcedir=data",
            "--sourcedir=data/ui",
            &format!("--target={}", target),
            "data/cargo-app.gresource.xml",
        ])
        .output()
        .expect("Failed to compile resources");

    if !output.status.success() {
        eprintln!(
            "glib-compile-resources failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
