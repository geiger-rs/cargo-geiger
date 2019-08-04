fn main() {
    unsafe { test2_package_with_shallow_deps::f() };
    println!("{}", num_cpus::get());
}
