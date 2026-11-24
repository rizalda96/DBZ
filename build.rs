// Embeds the DBZ icon into the Windows executable (the icon Explorer and shortcuts show).
// The resource is precompiled from assets/logo/dbz.rc, so no resource compiler is needed
// at build time. To regenerate after changing icon.ico, run from assets/logo:
//   llvm-windres -F pe-x86-64 dbz.rc dbz-res.o
//   llvm-rc -fo dbz.res dbz.rc
fn main() {
    let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/logo");
    let target = std::env::var("TARGET").unwrap_or_default();
    let res = if target.ends_with("windows-gnu") {
        dir.join("dbz-res.o")
    } else if target.ends_with("windows-msvc") {
        dir.join("dbz.res")
    } else {
        return;
    };
    println!("cargo:rerun-if-changed={}", res.display());
    println!("cargo:rustc-link-arg-bins={}", res.display());
}
