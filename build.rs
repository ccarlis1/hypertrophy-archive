use std::io;
use winres::WindowsResource;

fn main() -> io::Result<()> {
    if cfg!(target_os = "windows") {
        WindowsResource::new()
            .compile()?;
    }
    Ok(())
}