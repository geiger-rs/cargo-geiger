static mut N: usize = 0;

pub unsafe fn f() {
    test1_package_with_no_deps::f();
    test1_package_with_no_deps::g();

    N = N + 1;
}
