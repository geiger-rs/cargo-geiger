fn main() {
    unsafe { test2_package_with_shallow_deps::f() };
    
    use itertools::Itertools;
    let it = (1..3).interleave(vec![-1, -2]);
    itertools::assert_equal(it, vec![1, -1, 2, -2]);
}
