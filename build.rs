fn main() {
    #[cfg(windows)]
    compile_windows_resources();
}

#[cfg(windows)]
fn compile_windows_resources() {
    winres::WindowsResource::new()
        .compile()
        .expect("failed to compile Windows resources");
}
