use std::io::Write;

pub unsafe fn f() {
    unimplemented!()
}

pub fn g() {
    std::io::stdout().write_all(unsafe {
        std::str::from_utf8_unchecked(b"binarystring")
    }.as_bytes()).unwrap();
}

