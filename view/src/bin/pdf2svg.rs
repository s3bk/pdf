use env_logger;
use pdf::file::File as PdfFile;
use pdf::object::*;
use pdf::error::PdfError;
use std::env;
use std::fs;
use view::render_page;
use pathfinder_export::{Export, FileFormat};

fn main() -> Result<(), PdfError> {
    env_logger::init();
    
    let path = env::args().nth(1).expect("no file given");
    println!("read: {}", path);
    let file = PdfFile::<Vec<u8>>::open(&path)?;
    
    file.pages(|i, p| {
        let mut out = fs::File::create(format!("{}_{}.svg", path, i)).expect("can't create output file");
        render_page(&file, p).export(&mut out, FileFormat::SVG);
    }, 0 .. 3)?;
    Ok(())
}
