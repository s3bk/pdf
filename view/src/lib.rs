#[macro_use] extern crate log;
#[macro_use] extern crate pdf;
extern crate env_logger;

use std::io::Write;
use std::mem;
use std::convert::TryInto;
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;

use pdf::file::File as PdfFile;
use pdf::object::*;
use pdf::primitive::Primitive;
use pdf::backend::Backend;
use pdf::font::FontType;
use pdf::content::Operation;
use pdf::error::{PdfError, Result};

use pathfinder_content::color::ColorU;
use pathfinder_geometry::{
    vector::Vector2F, rect::RectF, transform2d::Transform2DF
};
use pathfinder_canvas::{CanvasRenderingContext2D, CanvasFontContext, Path2D, FillStyle};
use pathfinder_renderer::scene::Scene;
use font_kit::loaders::freetype::Font;
use euclid::Vector2D;
use skribo::{Glyph, Layout, FontRef};

macro_rules! ops_p {
    ($ops:ident, $($point:ident),* => $block:block) => ({
        let mut iter = $ops.iter();
        $(
            let x = iter.next().unwrap().as_number().unwrap();
            let y = iter.next().unwrap().as_number().unwrap();
            let $point = Vector2F::new(x, y);
        )*
        $block
    })
}
macro_rules! ops {
    ($ops:ident, $($var:ident : $typ:ty),* => $block:block) => ({
        || -> Result<()> {
            let mut iter = $ops.iter();
            $(
                let $var: $typ = iter.next().ok_or(PdfError::EOF)?.try_into()?;
            )*
            $block;
            Ok(())
        }();
    })
}

type P = Vector2F;
fn rgb2fill(r: f32, g: f32, b: f32) -> FillStyle {
    let c = |v: f32| (v * 255.) as u8;
    FillStyle::Color(ColorU { r: c(r), g: c(g), b: c(b), a: 255 })
}
fn gray2fill(g: f32) -> FillStyle {
    rgb2fill(g, g, g)
}
fn cymk2fill(c: f32, y: f32, m: f32, k: f32) -> FillStyle {
    rgb2fill(
        (1.0 - c) * (1.0 - k),
        (1.0 - m) * (1.0 - k),
        (1.0 - y) * (1.0 - k)
    )
}

#[derive(Clone)]
struct FontEntry {
    font: Font,
    subtype: FontType
}
enum TextMode {
    Fill,
    Stroke,
    FillThenStroke,
    Invisible,
    FillAndClip,
    StrokeAndClip
}
struct LineLayout<'a> {
    state: &'a TextState<'a>,
    font: FontRef,
    glyphs: Vec<Glyph>,
    scale: f32,
    advance: Vector2D<f32>,
}
impl<'a> LineLayout<'a> {
    fn new(state: &'a TextState) -> LineLayout<'a> {
        LineLayout {
            state,
            font: FontRef::new(state.font.font.clone()),
            glyphs: vec![],
            scale: state.font_size / (state.font.font.metrics().units_per_em as f32),
            advance: Vector2D::zero()
        }
    }
    fn add_char(&mut self, c: char) {
        let font = &self.state.font.font;
        if let Some(glyph_id) = font.glyph_for_char(c) {
            self.add_glyph(glyph_id);
            
            let dx = match c {
                ' ' => self.state.word_space,
                _   => self.state.char_space
            };
            self.advance.x += dx;
        }
    }
    fn add_glyph(&mut self, glyph_id: u32) {
        self.glyphs.push(Glyph {
            font: self.font.clone(),
            glyph_id,
            offset: self.advance
        });
        self.advance += self.state.font.font.advance(glyph_id).unwrap() * self.scale;
    }
    fn advance(&mut self, offset: f32) {
        self.advance.x += offset * self.state.font_size;
    }
    fn to_layout(self) -> Layout {
        Layout {
            size: self.state.font_size,
            glyphs: self.glyphs,
            advance: self.advance
        }
    }
}
struct TextState<'a> {
    text_matrix: Transform2DF, // tracks current glyph
    line_matrix: Transform2DF, // tracks current line
    char_space: f32, // Character spacing
    word_space: f32, // Word spacing
    horiz_scale: f32, // Horizontal scaling
    leading: f32, // Leading
    font: &'a FontEntry, // Text font
    font_size: f32, // Text font size
    mode: TextMode, // Text rendering mode
    rise: f32, // Text rise
    knockout: f32 //Text knockout
}
impl<'a> TextState<'a> {
    fn new(default_font: &FontEntry) -> TextState {
        TextState {
            text_matrix: Transform2DF::default(),
            line_matrix: Transform2DF::default(),
            char_space: 0.,
            word_space: 0.,
            horiz_scale: 1.,
            leading: 0.,
            font: default_font,
            font_size: 0.,
            mode: TextMode::Fill,
            rise: 0.,
            knockout: 0.
        }
    }
    fn translate(&mut self, v: Vector2F) {
        self.set_matrix(self.line_matrix.post_translate(v));
    }
    
    // move to the next line
    fn next_line(&mut self) {
        debug!("next line");
        self.translate(Vector2F::new(0., -self.leading * self.font_size));
    }
    // set text and line matrix
    fn set_matrix(&mut self, m: Transform2DF) {
        self.text_matrix = m;
        self.line_matrix = m;
    }
    fn draw_text(&mut self, canvas: &mut CanvasRenderingContext2D, text: &str) {
        let mut layout = LineLayout::new(self);
        for c in text.chars() {
            layout.add_char(c);
        }
        self.draw_layout(canvas, layout.to_layout());
    }
    fn advance(&mut self, v: Vector2F) {
        self.text_matrix = self.text_matrix.post_translate(v);
    }
    fn draw_layout(&mut self, canvas: &mut CanvasRenderingContext2D, layout: Layout) {
        let transform = Transform2DF::row_major(self.horiz_scale, 0., 0., -1.0, 0., self.rise)
            .post_mul(&self.text_matrix);
        debug!("transform: {:?}", transform);
        
        let advance = layout.advance;
        canvas.fill_layout(&layout, transform);
        self.advance(Vector2F::new(advance.x * self.horiz_scale, 0.));
    }
}

fn render_text<'a, I>(iter: & mut I, canvas: & mut CanvasRenderingContext2D, fonts: &'a HashMap<&'a str, FontEntry>, default_font: &'a FontEntry) 
    where I: Iterator<Item=&'a Operation>
{
    let mut state = TextState::new(default_font);
    
    while let Some(op) = iter.next() {
        debug!("{}", op);
        let ref ops = op.operands;
        match op.operator.as_str() {
            "ET" => break,
            
            // state modifiers
            
            // character spacing
            "Tc" => ops!(ops, char_space: f32 => {
                    state.char_space = char_space;
            }),
            
            // word spacing
            "Tw" => ops!(ops, word_space: f32 => {
                    state.word_space = word_space;
            }),
            
            // Horizontal scaling (in percent)
            "Tz" => ops!(ops, scale: f32 => {
                    state.horiz_scale = 0.01 * scale;
            }),
            
            // leading
            "TL" => ops!(ops, leading: f32 => {
                    state.leading = leading;
            }),
            
            // text font
            "Tf" => ops!(ops, font: &str, size: f32 => {
                if let Some(e) = fonts.get(font) {
                    canvas.set_font(e.font.clone());
                    state.font = e;
                    debug!("new font: {}", e.font.full_name());
                }
                canvas.set_font_size(size);
                state.font_size = size;
            }),
            
            // render mode
            "Tr" => ops!(ops, mode: i32 => {
                use TextMode::*;
                state.mode = match mode {
                    0 => Fill,
                    1 => Stroke,
                    2 => FillThenStroke,
                    3 => Invisible,
                    4 => FillAndClip,
                    5 => StrokeAndClip,
                    _ => {
                        return Err(PdfError::Other { msg: format!("Invalid text render mode: {}", mode)});
                    }
                }
            }),
            
            // text rise
            "Ts" => ops!(ops, rise: f32 => {
                state.rise = rise;
            }),
            
            // positioning operators
            // Move to the start of the next line
            "Td" => ops_p!(ops, t => {
                state.translate(t);
            }),
            
            "TD" => ops_p!(ops, t => {
                state.leading = -t.x();
                state.translate(t);
            }),
            
            // Set the text matrix and the text line matrix
            "Tm" => ops!(ops, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32 => {
                state.set_matrix(Transform2DF::row_major(a, b, c, d, e, f));
            }),
            
            // Move to the start of the next line
            "T*" => {
                state.next_line();
            },
            
            // draw text
            "Tj" => ops!(ops, text: &str => {
                state.draw_text(canvas, text);
            }),
            
            // move to the next line and draw text
            "'" => ops!(ops, text: &str => {
                state.next_line();
                state.draw_text(canvas, text);
            }),
            
            // set word and charactr spacing, move to the next line and draw text
            "\"" => ops!(ops, word_space: f32, char_space: f32, text: &str => {
                state.word_space = word_space;
                state.char_space = char_space;
                state.next_line();
                state.draw_text(canvas, text);
            }),
            "TJ" => ops!(ops, array: &[Primitive] => {
                let mut layout = LineLayout::new(&state);
                
                for arg in array {
                    match arg {
                        Primitive::String(ref data) => {
                            for &b in data.as_bytes() {
                                layout.add_char(b as char);
                            }
                        },
                        p => {
                            let offset = p.as_number().expect("wrong argument to TJ");
                            layout.advance(-0.001 * offset); // because why not PDF…
                        }
                    }
                }
                state.draw_layout(canvas, layout.to_layout());
            }),
            _ => {}
        }
    }
}

pub fn render_page<B: Backend>(file: &PdfFile<B>, page: &Page) -> Scene {
    let Rect { left, right, top, bottom } = page.media_box(file).expect("no media box");
    
    let resources = page.resources(file);
    
    let rect = RectF::from_points(Vector2F::new(left, bottom), Vector2F::new(right, top));
    
    let mut canvas = CanvasRenderingContext2D::new(CanvasFontContext::from_system_source(), rect.size());
    canvas.stroke_rect(RectF::new(Vector2F::default(), rect.size()));
    let root_tansformation = Transform2DF::row_major(1.0, 0.0, 0.0, -1.0, -left, top);
    canvas.set_current_transform(&root_tansformation);
    debug!("transform: {:?}", canvas.current_transform());
    
    let font_path = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("fonts/MinionPro-Regular.otf");
    info!("font path: {:?}", &font_path);
    
    let font = Font::from_path(font_path, 0).expect("can't open font");
    canvas.set_font(font.clone());
    let default_font = FontEntry {
        font,
        subtype: FontType::TrueType
    };
    
    let mut fonts = HashMap::new();
    for (name, font) in resources.iter().flat_map(|r| r.fonts()) {
        debug!("{} -> {:?}", name, font);
        if let Some(Ok(data)) = font.data() {
            ::std::fs::File::create(&format!("/tmp/font_{}", name)).unwrap().write_all(data).unwrap();
            fonts.insert(
                name,
                FontEntry {
                    font: Font::from_bytes(Arc::new(data.into()), 0)
                        .expect("failed to load embedded font"),
                    subtype: font.subtype
                }
            );
        }
    }
    
    let mut path = Path2D::new();
    let mut last = Vector2F::default();
    
    let mut iter = page.contents.as_ref().expect("no contents").operations.iter();
    while let Some(op) = iter.next() {
        debug!("transform: {:?}", canvas.current_transform());
        debug!("{}", op);
        let ref ops = op.operands;
        match op.operator.as_str() {
            "m" => { // move x y
                ops_p!(ops, p => {
                    path.move_to(p);
                    last = p;
                })
            }
            "l" => { // line x y
                ops_p!(ops, p => {
                    path.line_to(p);
                    last = p;
                })
            }
            "c" => { // cubic bezier c1.x c1.y c2.x c2.y p.x p.y
                ops_p!(ops, c1, c2, p => {
                    path.bezier_curve_to(c1, c2, p);
                    last = p;
                })
            }
            "v" => { // cubic bezier c2.x c2.y p.x p.y
                ops_p!(ops, c2, p => {
                    path.bezier_curve_to(last, c2, p);
                    last = p;
                })
            }
            "y" => { // cubic c1.x c1.y p.x p.y
                ops_p!(ops, c1, p => {
                    path.bezier_curve_to(c1, p, p);
                    last = p;
                })
            }
            "h" => { // close
                path.close_path();
            }
            "re" => { // rect x y width height
                ops_p!(ops, origin, size => {
                    let r = RectF::new(origin, size);
                    path.rect(r);
                })
            }
            "S" => { // stroke
                canvas.stroke_path(mem::replace(&mut path, Path2D::new()));
            }
            "s" => { // close and stroke
                path.close_path();
                canvas.stroke_path(mem::replace(&mut path, Path2D::new()));
            }
            "f" | "F" | "f*" => { // close and fill 
                // TODO: implement windings
                path.close_path();
                canvas.fill_path(mem::replace(&mut path, Path2D::new()));
            }
            "B" | "B*" => { // fill and stroke
                path.close_path();
                let path2 = mem::replace(&mut path, Path2D::new());
                canvas.fill_path(path2.clone());
                canvas.stroke_path(path2);
            }
            "b" | "b*" => { // stroke and fill
                path.close_path();
                let path2 = mem::replace(&mut path, Path2D::new());
                canvas.stroke_path(path2.clone());
                canvas.fill_path(path2);
            }
            "n" => { // clear path
                path = Path2D::new();
            }
            "q" => { // save state
                canvas.save();
            }
            "Q" => { // restore
                canvas.restore();
            }
            "cm" => { // modify transformation matrix 
                ops!(ops, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32 => {
                    let tr = canvas.current_transform().pre_mul(
                        &Transform2DF::row_major(a, b, c, d, e, f)
                    );
                    canvas.set_current_transform(&tr);
                })
            }
            "w" => { // line width
                ops!(ops, width: f32 => {
                    canvas.set_line_width(width);
                })
            }
            "J" => { // line cap
            }
            "j" => { // line join 
            }
            "M" => { // miter limit
            }
            "d" => { // line dash [ array phase ]
            }
            "gs" => { // set from graphic state dictionary
            },
            "W" | "W*" => { // clipping path
            }
            "SC" | "RG" => { // stroke color
                ops!(ops, r: f32, g: f32, b: f32 => {
                    canvas.set_stroke_style(rgb2fill(r, g, b));
                });
            }
            "sc" | "rg" => { // fill color
                ops!(ops, r: f32, g: f32, b: f32 => {
                    canvas.set_fill_style(rgb2fill(r, g, b));
                });
            }
            "G" => { // stroke gray
                ops!(ops, gray: f32 => {
                    canvas.set_stroke_style(gray2fill(gray));
                })
            }
            "g" => { // stroke gray
                ops!(ops, gray: f32 => {
                    canvas.set_fill_style(gray2fill(gray));
                })
            }
            "k" => { // fill color
                ops!(ops, c: f32, y: f32, m: f32, k: f32 => {
                    canvas.set_fill_style(cymk2fill(c, y, m, k));
                });
            }
            "cs" => { // color space
            }
            "BT" => {
                let graphics_transformation = canvas.current_transform();
                render_text(&mut iter, &mut canvas, &fonts, &default_font);
            }
            _ => {}
        }
    }
    
    canvas.into_scene()
}

