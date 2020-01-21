fn main() {
    println!("cargo:rustc-link-lib=static=WMM");
    println!("cargo:rustc-link-search=native=.");
}