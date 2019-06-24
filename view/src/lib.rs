#[macro_use] extern crate log;
extern crate env_logger;

use std::env::args;
use std::time::SystemTime;
use std::fs;
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

use pathfinder_geometry::outline::{Contour, Outline};
use pathfinder_geometry::basic::vector::Vector2F;
use pathfinder_geometry::basic::rect::RectF;
use pathfinder_geometry::basic::transform2d::Transform2DF;
use pathfinder_geometry::color::ColorU;
use pathfinder_canvas::{CanvasRenderingContext2D, CanvasFontContext, Path2D, FillStyle};
use pathfinder_renderer::scene::Scene;
use font_kit::loaders::freetype::Font;
use font_kit::hinting::HintingOptions;
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
        let mut iter = $ops.iter();
        $(
            let $var: $typ = iter.next().unwrap().try_into().unwrap();
        )*
        $block
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

fn render_text<'a, I>(iter: & mut I, canvas: & mut CanvasRenderingContext2D, fonts: &'a HashMap<&'a str, FontEntry>, mut current_font: &'a FontEntry) 
    where I: Iterator<Item=&'a Operation>
{
    let mut current_line = Vector2F::default();
    let mut current_char = current_line;
    let mut font_size = 0.0;
    
    while let Some(op) = iter.next() {
        debug!("{}", op);
        let ref ops = op.operands;
        match op.operator.as_str() {
            "ET" => break,
            "Tf" => { // text font
                ops!(ops, font: &str, size: f32 => {
                    if let Some(e) = fonts.get(font) {
                        canvas.set_font(e.font.clone());
                        current_font = e;
                        debug!("new font: {}", e.font.full_name());
                    }
                    canvas.set_font_size(size);
                    font_size = size;
                });
            }
            "Tj" => { // draw text
                ops!(ops, text: &str => {
                    canvas.fill_text(text, current_char + Vector2F::new(0.0, font_size));
                    current_char = current_char + Vector2F::new(canvas.measure_text(text).width, 0.);
                });
            }
            "TJ" => {
                let arr = ops[0].as_array().expect("not an array");
                let mut glyphs = Vec::new();
                let mut advance = Vector2D::zero();
                let font = FontRef::new(current_font.font.clone());
                
                debug!("current font: {}", current_font.font.full_name());
                let scale = font_size / (current_font.font.metrics().units_per_em as f32);
                for arg in arr {
                    match arg {
                        Primitive::String(ref data) => {
                            for &b in data.as_bytes() {
                                debug!("char: {} {}", b, b as char);
                                if let Some(glyph_id) = current_font.font.glyph_for_char(b as char) {
                                    glyphs.push(Glyph {
                                        font: font.clone(),
                                        glyph_id,
                                        offset: advance
                                    });
                                    advance += current_font.font.advance(glyph_id).unwrap() * scale;
                                }
                            }
                            //let advance = current_font.advance(b as u32).expect("can't get advance");
                            //Vector2F::new(advance.x, advance.y) * 
                            //transform = transform.post_transform(&Vector2F::new(canvas.measure_text(text).width, 0.));
                        },
                        p => {
                            let offset = p.as_number().expect("wrong argument to TJ");
                            advance.x -= font_size * 0.001 * offset; // because why not PDF…
                        }
                    }
                }
                let transform = Transform2DF::from_scale(Vector2F::new(1.0, -1.0))
                    .post_mul(&Transform2DF::from_translation(current_line));
                
                canvas.fill_layout(&Layout {
                    size: font_size,
                    glyphs,
                    advance
                }, transform);
                current_char = current_char + Vector2F::new(advance.x, advance.y);
            },
            "Td" => { // move current line
                ops_p!(ops, t => {
                    current_line = current_line + t;
                    current_char = current_line;
                });
            },
            _ => {}
        }
    }
    
    canvas.restore();
}

pub fn render_page<B: Backend>(file: &PdfFile<B>, page: &Page) -> Scene {
    let Rect { left, right, top, bottom } = page.media_box(file).expect("no media box");
    
    let resources = page.resources(file).expect("no resources");
    
    let rect = RectF::from_points(Vector2F::new(left, bottom), Vector2F::new(right, top));
    
    let mut canvas = CanvasRenderingContext2D::new(CanvasFontContext::from_system_source(), rect.size());
    canvas.stroke_rect(RectF::new(Vector2F::default(), rect.size()));
    canvas.set_current_transform(&Transform2DF::row_major(1.0, 0.0, 0.0, -1.0, -left, top));
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
    for (name, font) in resources.fonts() {
        dbg!((&name, &font));
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
            "BT" => render_text(&mut iter, &mut canvas, &fonts, &default_font),
            _ => {}
        }
    }
    
    canvas.into_scene()
}

