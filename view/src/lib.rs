#[macro_use] extern crate log;
extern crate pdf;
extern crate env_logger;

use std::io::Write;
use std::mem;
use std::convert::TryInto;
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use std::rc::Rc;

use pdf::file::File as PdfFile;
use pdf::object::*;
use pdf::primitive::Primitive;
use pdf::backend::Backend;
use pdf::font::Font as PdfFont;
use pdf::content::Operation;
use pdf::error::{PdfError, Result};
use pdf::encoding::{Encoding, Decoder};

use pathfinder_content::color::ColorU;
use pathfinder_geometry::{
    vector::Vector2F, rect::RectF, transform2d::Transform2DF
};
use pathfinder_canvas::{CanvasRenderingContext2D, CanvasFontContext, Path2D, FillStyle};
use pathfinder_renderer::scene::Scene;
use euclid::Vector2D;
use font::Font;

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
    font: Box<Font>,
    subtype: FontType,
    decoder: Decoder,
    widths: Box<[f32; 256]>
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
    font: &'a FontEntry,
    fontref: FontRef,
    glyphs: Vec<Glyph>,
    scale: f32,
    advance: Vector2D<f32>,
}
impl<'a> LineLayout<'a> {
    fn new(state: &'a TextState, font: &'a FontEntry) -> LineLayout<'a> {
        LineLayout {
            state,
            font,
            fontref: FontRef::new(font.font.clone()),
            glyphs: vec![],
            scale: state.font_size / (font.font.metrics().units_per_em as f32),
            advance: Vector2D::zero()
        }
    }
    
    fn add_bytes_cid(&mut self, data: &[u8])
    
    fn add_bytes(&mut self, data: &[u8]) {
        if self.font.is_cid {
            return self.add_bytes_cid(bytes);
        }
        
        let font = &self.font.font;
        for b in data.bytes() {
            if let Some(glyph_id) = font.glyph_for_char(b as char) {
                self.glyphs.push(Glyph {
                    font: self.fontref.clone(),
                    glyph_id,
                    offset: self.advance
                });
                
            } else {
                info!("{}: can't find char 0x{:02X}", self.font.font.full_name(), b);
            }
            
            let dx = match b {
                b' ' => self.state.word_space,
                _   => self.state.char_space
            };
            let glyph_width = self.font.widths[b as usize];
            if glyph_width == 0.0 {
                info!("No glyph width for char 0x{:02X}", b);
            }
            self.advance.x += dx + glyph_width * self.scale;
        }
    }
    fn advance(&mut self, offset: f32) {
        self.advance.x += offset;
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
    font: Option<&'a FontEntry>, // Text font
    font_size: f32, // Text font size
    mode: TextMode, // Text rendering mode
    rise: f32, // Text rise
    knockout: f32 //Text knockout
}
impl<'a> TextState<'a> {
    fn new() -> TextState<'a> {
        TextState {
            text_matrix: Transform2DF::default(),
            line_matrix: Transform2DF::default(),
            char_space: 0.,
            word_space: 0.,
            horiz_scale: 1.,
            leading: 0.,
            font: None,
            font_size: 0.,
            mode: TextMode::Fill,
            rise: 0.,
            knockout: 0.
        }
    }
    fn translate(&mut self, v: Vector2F) {
        let m = Transform2DF::from_translation(v).post_mul(&self.line_matrix);
        self.set_matrix(m);
    }
    
    // move to the next line
    fn next_line(&mut self) {
        self.translate(Vector2F::new(0., -self.leading * self.font_size));
    }
    // set text and line matrix
    fn set_matrix(&mut self, m: Transform2DF) {
        self.text_matrix = m;
        self.line_matrix = m;
    }
    fn draw_text(&mut self, canvas: &mut CanvasRenderingContext2D, text: &[u8]) {
        if let Some(font) = self.font {
            let mut layout = LineLayout::new(self, font);
            for &b in text {
                layout.add_byte(b);
            }
            let layout = layout.to_layout();
            self.draw_layout(canvas, layout);
        }
    }
    fn advance(&mut self, v: Vector2F) {
        self.text_matrix = Transform2DF::from_translation(v).post_mul(&self.text_matrix);
    }
    fn draw_layout(&mut self, canvas: &mut CanvasRenderingContext2D, layout: Layout) {
        let transform = Transform2DF::row_major(self.horiz_scale, 0., 0., -1.0, 0., self.rise)
            .post_mul(&self.text_matrix);
        
        let advance = layout.advance;
        canvas.fill_layout(&layout, transform);
        self.advance(Vector2F::new(advance.x * self.horiz_scale, 0.));
    }
}

pub struct Cache {
    // shared mapping of fontname -> font
    fonts: HashMap<String, FontEntry>
}
impl Cache {
    pub fn new() -> Cache {
        Cache {
            fonts: HashMap::new()
        }
    }
    fn load_built_in_font(&mut self, font: &PdfFont) -> Option<Box<Font>> {
        font.standard_font().map(|filename| {
            let font_path = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
                .join("fonts")
                .join(filename);
            let data = fs::read(font_path).unwrap();
            match filename.rsplit(".").nth(0).unwrap() {
                "otf" => font::opentype(data),
                "ttf" => font::truetype(data),
                "PFB" => font::type1(data),
                e => panic!("unknown file extension .{}", ext)
            }
        })
    }
    fn load_font(&mut self, pdf_font: &PdfFont) {
        if self.fonts.get(&pdf_font.name).is_some() {
            return;
        }
        dbg!(pdf_font);
        let mut font = match (self.load_built_in_font(&pdf_font), pdf_font.data()) {
            (_, Some(Ok(data))) => {
                let ext = match pdf_font.subtype {
                    FontType::Type1 | FontType::CIDFontType0 => ".pfb",
                    FontType::TrueType | FontType::CIDFontType2 => ".ttf",
                    _ => "",
                };
                ::std::fs::File::create(&format!("/tmp/fonts/{}{}", pdf_font.name, ext)).unwrap().write_all(data).unwrap();
                
                match pdf_font.subtype {
                    FontType::TrueType | FontType::CIDFontType2 => TrueTypeFont::parse(data, 0)
                        .expect("can't parse truetype font"),
                    FontType::CIDFontType0 => CffFont::parse(data, 0).expect("can't parse CFF font")
                    t => panic!("Fonttype {:?} not yet implemented")
                }
            }
            (Some(f), _) => f,
            (None, Some(Err(e))) => panic!("can't decode font data: {:?}", e),
            (None, None) => {
                dbg!(font);
                warn!("No font data for {}. Glyphs will be missing.", pdf_font.name);
                return;
            }
        };
        
        let widths = match pdf_font.widths() {
            Ok(Some(widths)) => widths,
            Err(e) => {
                error!("can't get font widths: {:?}", e);
                return;
            }
            Ok(None) => {
                let mut widths = [0.0; 256];
                warn!("Font {} without widhts", pdf_font.name);
                for b in 0u8 ..= 255 {
                    if let Some(glyph) = ft_font.glyph_for_char(b as char) {
                        if let Ok(v) = ft_font.advance(glyph) {
                            widths[b as usize] = v.x;
                        }
                    }
                }
                widths
            }
        };
        
        let is_cid = match pdf_font.subtype {
            FontType::CIDFontType0 || FontType::CIDFontType2 => true,
            _ => false
        };
            
        self.fonts.insert(font.name.clone(), FontEntry {
            font,
            subtype: font.subtype,
            decoder: Decoder::new(encoding),
            widths: Box::new(widths),
            is_cid
        });
    }
    fn get_font(&self, font_name: &str) -> Option<&FontEntry> {
        self.fonts.get(font_name)
    }
    
    pub fn render_page<B: Backend>(&mut self, file: &PdfFile<B>, page: &Page) -> Result<Scene> {
        let Rect { left, right, top, bottom } = page.media_box(file).expect("no media box");
        
        let resources = page.resources(file)?;
        
        let rect = RectF::from_points(Vector2F::new(left, bottom), Vector2F::new(right, top));
        
        let mut canvas = CanvasRenderingContext2D::new(CanvasFontContext::from_system_source(), rect.size());
        canvas.stroke_rect(RectF::new(Vector2F::default(), rect.size()));
        let root_tansformation = Transform2DF::row_major(1.0, 0.0, 0.0, -1.0, -left, top);
        canvas.set_current_transform(&root_tansformation);
        debug!("transform: {:?}", canvas.current_transform());
        
        // make sure all fonts are in the cache, so we can reference them
        for font in resources.fonts.values() {
            self.load_font(font);
        }
        for gs in resources.graphics_states.values() {
            if let Some((ref font, _)) = gs.font {
                self.load_font(font);
            }
        }
        
        let mut path = Path2D::new();
        let mut last = Vector2F::default();
        let mut state = TextState::new();
        
        let mut iter = page.contents.as_ref()?.operations.iter();
        while let Some(op) = iter.next() {
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
                "gs" => ops!(ops, gs: &str => { // set from graphic state dictionary
                    let gs = resources.graphics_states.get(gs)?;
                    
                    if let Some(lw) = gs.line_width {
                        canvas.set_line_width(lw);
                    }
                    if let Some((ref font, size)) = gs.font {
                        if let Some(e) = self.get_font(&font.name) {
                            canvas.set_font(e.font.clone());
                            canvas.set_font_size(size);
                            state.font = Some(e);
                            state.font_size = size;
                            debug!("new font: {} at size {}", e.font.full_name(), size);
                        } else {
                            state.font = None;
                        }
                    }
                }),
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
                    state = TextState::new();
                }
                "ET" => {
                    state.font = None;
                }
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
                "Tf" => ops!(ops, font_name: &str, size: f32 => {
                    let font = resources.fonts.get(font_name)?;
                    if let Some(e) = self.get_font(&font.name) {
                        canvas.set_font(e.font.clone());
                        state.font = Some(e);
                        debug!("new font: {}", e.font.full_name());
                        canvas.set_font_size(size);
                        state.font_size = size;
                    } else {
                        state.font = None;
                    }
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
                    state.leading = -t.y();
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
                "Tj" => ops!(ops, text: &[u8] => {
                    state.draw_text(&mut canvas, text);
                }),
                
                // move to the next line and draw text
                "'" => ops!(ops, text: &[u8] => {
                    state.next_line();
                    state.draw_text(&mut canvas, text);
                }),
                
                // set word and charactr spacing, move to the next line and draw text
                "\"" => ops!(ops, word_space: f32, char_space: f32, text: &[u8] => {
                    state.word_space = word_space;
                    state.char_space = char_space;
                    state.next_line();
                    state.draw_text(&mut canvas, text);
                }),
                "TJ" => ops!(ops, array: &[Primitive] => {
                    if let Some(font) = state.font {
                        let mut layout = LineLayout::new(&state, font);
                        let mut text: Vec<u8> = Vec::new();
                        for arg in array {
                            match arg {
                                Primitive::String(ref data) => {
                                    layout.add_bytes(data.as_bytes());
                                    text.extend(data.as_bytes());
                                },
                                p => {
                                    let offset = p.as_number().expect("wrong argument to TJ");
                                    layout.advance(-0.001 * offset); // because why not PDF…
                                }
                            }
                        }
                        debug!("Text: {}", font.decoder.decode_bytes(&text));
                        let layout = layout.to_layout();
                        state.draw_layout(&mut canvas, layout);
                    }
                }),
                _ => {}
            }
        }
        
        Ok(canvas.into_scene())
    }
}
