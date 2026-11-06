use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/rustybox.rc");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        let out_dir = env::var("OUT_DIR").unwrap();
        let res_path = Path::new(&out_dir).join("rustybox.res");

        let windres = if Command::new("x86_64-w64-mingw32-windres")
            .arg("--version")
            .output()
            .is_ok()
        {
            "x86_64-w64-mingw32-windres"
        } else {
            "windres"
        };

        let status = Command::new(windres)
            .args(&[
                "assets/rustybox.rc",
                "-O",
                "coff",
                "-o",
                res_path.to_str().unwrap(),
            ])
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg={}", res_path.display());
            }
        }
    }
}
