use std::error::Error;
use otf::cff::{Cff, glyphs::{Glyphs, charstring::{Operation, Point}}, error::CffError};
use sfnt::{Sfnt};
use pathfinder_canvas::Path2D;
use pathfinder_geometry::vector::Vector2F;
use crate::Font;

pub struct CffFont<'a> {
    glyphs: Glyphs<'a>
}

fn convert_err(e: CffError) -> Box<dyn Error> {
    format!("{:?}", e).into()
}

impl<'a> CffFont<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        let cff = Cff::parse(&data).map_err(convert_err)?;
        let glyphs = cff.parse_glyphs(0).unwrap().unwrap();
        Ok(CffFont { glyphs })
    }
    pub fn parse_opentype(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        // Parse the font file and find the CFF table in the font file.
        let sfnt = Sfnt::parse(&data).unwrap();
        for (r, _) in sfnt.tables() {
            println!("{:?}", std::str::from_utf8(&*r.tag));
        }
        let (_, data) = sfnt.find(b"CFF ").unwrap();
        dbg!(&data[..100]);
        Self::parse(data)
    }
}
impl<'a> Font for CffFont<'a> {
    fn num_glyphs(&self) -> u32 {
        self.glyphs.charstrings.len() as u32
    }
    fn glyph(&self, id: u32) -> Result<Path2D, Box<dyn Error>> {
        // Find the charstring for the ".notdef" glyph.
        let (charstring, _) = self.glyphs.parse_charstring(id as usize).unwrap().unwrap();

        let mut path = Path2D::new();
        let v = |p: Point| Vector2F::new(p.x as f32, p.y as f32);
        
        // Parse and collect the operations in the charstring.
        for op in charstring.operations() {
            match op.map_err(convert_err)? {
                Operation::MoveTo(p) => path.move_to(v(p)),
                Operation::LineTo(p) => path.line_to(v(p)),
                Operation::CurveTo(c1, c2, p) => path.bezier_curve_to(v(c1), v(c2), v(p)),
                _ => {}
            }
        }
        
        Ok(path)
    }
}
