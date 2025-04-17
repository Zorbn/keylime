use std::path::Path;

fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest = Path::new("extra/keylime.exe.manifest")
            .canonicalize()
            .unwrap();

        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rerun-if-changed=keylime.exe.manifest");
    }

    #[cfg(target_os = "macos")]
    {
        let info = Path::new("extra/Info.plist").canonicalize().unwrap();

        println!(
            "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            info.display()
        );
        println!("cargo:rerun-if-changed=Info.plist");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
