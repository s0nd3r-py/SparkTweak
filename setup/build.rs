// Установщик встраивает уже собранный sparktweak.exe из той же папки target/<profile>.
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile_dir = out.ancestors().nth(3).expect("target/<profile>");
    let exe = profile_dir.join("sparktweak.exe");
    println!("cargo:rerun-if-changed={}", exe.display());
    if !exe.exists() {
        panic!("Не найден {}. Сначала соберите приложение: cargo build --release -p sparktweak", exe.display());
    }
    println!("cargo:rustc-env=SPARK_APP_EXE={}", exe.display());
}
