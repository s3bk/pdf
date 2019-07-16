use std::error::Error;
use pathfinder_canvas::Path2D;
use pathfinder_geometry::vector::Vector2F;
use rusttype::{GlyphId};
use stb_truetype::VertexType;
use crate::Font;

pub struct TrueTypeFont<'a> {
    font: rusttype::Font<'a>
}
impl<'a> TrueTypeFont<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        let font = rusttype::Font::from_bytes(data)?;
        Ok(TrueTypeFont { font })
    }
}
impl<'a> Font for TrueTypeFont<'a> {
    fn num_glyphs(&self) -> u32 {
        self.font.glyph_count() as u32
    }
    fn glyph(&self, id: u32) -> Result<Path2D, Box<dyn Error>> {
        let glyph_scale = 1. / self.font.units_per_em() as f32;
        let scale_vector = Vector2F::new(glyph_scale, glyph_scale);
        
        let mut path = Path2D::new();
    
        let glyph = self.font.glyph(GlyphId(id)).standalone();
        if let Some(shape) = glyph.get_data().as_ref().and_then(|data| data.shape.as_ref()) {
            for vertex in shape {
                let p = Vector2F::new(vertex.x as _, vertex.y as _) * scale_vector;
                
                match vertex.vertex_type() {
                    VertexType::MoveTo => path.move_to(p),
                    VertexType::LineTo => path.line_to(p),
                    VertexType::CurveTo => {
                        let c = Vector2F::new(vertex.cx as _, vertex.cy as _) * scale_vector;
                        path.quadratic_curve_to(c, p);
                    }
                }
            }
            path.close_path();
        }
        
        Ok(path)
    }
}
