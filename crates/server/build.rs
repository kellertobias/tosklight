fn main() {
    println!("cargo:rerun-if-changed=../../apps/control-ui/dist");
    println!("cargo:rerun-if-changed=../../apps/control-ui/dist/index.html");
}
