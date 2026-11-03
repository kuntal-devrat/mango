use std::path::Path;

fn main() {
    #[cfg(target_os = "linux")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let out_path = Path::new(&out_dir);
        let link_target = out_path.join("libfreetype.so");

        // Candidates for system libfreetype.so.6 on various Linux distributions.
        let candidates = [
            "/usr/lib/x86_64-linux-gnu/libfreetype.so.6",
            "/usr/lib64/libfreetype.so.6",
            "/usr/lib/libfreetype.so.6",
            "/usr/lib/aarch64-linux-gnu/libfreetype.so.6",
        ];

        for candidate in &candidates {
            let p = Path::new(candidate);
            if p.exists() {
                if !link_target.exists() {
                    let _ = std::os::unix::fs::symlink(p, &link_target);
                }
                println!("cargo:rustc-link-search=native={}", out_dir);
                println!("cargo:rustc-link-lib=dylib=freetype");
                break;
            }
        }
    }
}
