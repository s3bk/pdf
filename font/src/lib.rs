use std::error::Error;
use pathfinder_canvas::Path2D;

pub trait Font {
    fn num_glyphs(&self) -> u32;
    fn glyph(&self, id: u32) -> Result<Path2D, Box<dyn Error>>;
}

mod truetype;
mod cff;

pub use truetype::TrueTypeFont;
pub use cff::CffFont;
