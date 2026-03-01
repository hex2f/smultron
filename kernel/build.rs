use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().expect("workspace root");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let target_spec = manifest_dir.join("x86_64-smultron.json");

    build_user_app(root, &target_spec, "init", "0x0000555500000000");
    build_user_app(root, &target_spec, "echo", "0x0000555500100000");

    copy_artifact(root, "init", &out_dir.join("init.elf"));
    copy_artifact(root, "echo", &out_dir.join("echo.elf"));

    println!(
        "cargo:rerun-if-changed={}",
        root.join("userspace/apps/init").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("userspace/apps/echo").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("userspace/libos").display()
    );
}

fn build_user_app(root: &Path, target_spec: &Path, pkg: &str, image_base: &str) {
    let userspace_target_dir = root.join("target/userspace");
    let status = Command::new("cargo")
        .arg("rustc")
        .arg("-p")
        .arg(pkg)
        .arg("--bin")
        .arg(pkg)
        .arg("--target")
        .arg(target_spec)
        .arg("-Z")
        .arg("build-std=core,alloc")
        .arg("-Z")
        .arg("json-target-spec")
        .arg("--")
        .arg("-C")
        .arg(format!("link-arg=--image-base={image_base}"))
        .env("CARGO_TARGET_DIR", &userspace_target_dir)
        .current_dir(root)
        .status()
        .expect("failed to invoke cargo rustc for userspace app");

    if !status.success() {
        panic!("failed to build userspace app: {}", pkg);
    }
}

fn copy_artifact(root: &Path, name: &str, out: &Path) {
    let src = root
        .join("target/userspace/x86_64-smultron/debug")
        .join(name);
    fs::copy(&src, out).unwrap_or_else(|e| panic!("copy {} failed: {}", src.display(), e));
}
