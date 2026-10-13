fn main() {
    println!("cargo:rerun-if-changed=../../boot/linker/x86_64-type1.ld");
}
