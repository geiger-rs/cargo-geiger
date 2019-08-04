fn main() {
    let bytes = b"string";
    let ptr = bytes as *const u8;
    unsafe { *ptr };
}
