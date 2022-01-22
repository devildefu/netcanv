use std::path::Path;
use std::process::Command;

/// Checks if cargo-about is available.
fn cargo_about_is_available() -> bool {
   Command::new("cargo-about").arg("--version").output().is_ok()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
   // Builds the license file using cargo-about.
   if cargo_about_is_available() {
      println!("cargo:rerun-if-changed=src/assets/about/about.toml");
      println!("cargo:rerun-if-changed=src/assets/about/about.hbs");
      println!("cargo:rerun-if-changed=Cargo.toml");

      let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
      let about_dir = src_dir.join("assets").join("about");
      let about = Command::new("cargo")
         .arg("about")
         .arg("generate")
         .arg(about_dir.join("about.hbs"))
         .arg("--config")
         .arg(about_dir.join("about.toml"))
         .output()?
         .stdout;
      std::fs::write(about_dir.join("about.html"), &about)?;
   } else {
      println!(
         "Warning: cargo-about is not available. Licensing information will not be available."
      );
   }

   Ok(())
}
