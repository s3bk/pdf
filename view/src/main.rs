use std::env::args;
use std::time::SystemTime;
use std::fs;
use std::io::Write;
use std::mem;

use pdf::file::File as PdfFile;
use pdf::object::*;

use pathfinder_geometry::outline::{Contour, Outline};
use pathfinder_geometry::basic::vector::Vector2F;
use pathfinder_geometry::basic::rect::RectF;
use pathfinder_geometry::basic::transform2d::Transform2DF;
use pathfinder_geometry::color::ColorU;
use pathfinder_canvas::{CanvasRenderingContext2D, CanvasFontContext, Path2D, FillStyle};
use pathfinder_renderer::scene::Scene;

macro_rules! run {
    ($e:expr) => (
        match $e {
            Ok(r) => r,
            Err(e) => return e.trace()
        }
    )
}

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
    ($ops:ident, $($var:ident),* => $block:block) => ({
        let mut iter = $ops.iter();
        $(
            let $var = iter.next().unwrap().as_number().unwrap();
        )*
        $block
    })
}

fn rgb2fill(r: f32, g: f32, b: f32) -> FillStyle {
    let c = |v: f32| (v * 255.) as u8;
    FillStyle::Color(ColorU { r: c(r), g: c(g), b: c(b), a: 255 })
}

fn render_page(page: &Page) -> Scene {
    let Rect { left, right, top, bottom } = page.media_box.expect("page has no media box");
    dbg!((left, right, top, bottom));
    let rect = RectF::from_points(Vector2F::new(left, bottom), Vector2F::new(right, top));
    dbg!(rect);
    
    let tr = |p: Vector2F| Vector2F::new(p.x(), top - p.y());
    let mut canvas = CanvasRenderingContext2D::new(CanvasFontContext::new(), rect.size());
    
    let mut path = Path2D::new();
    let mut last = Vector2F::default();
    for op in page.contents.iter().flat_map(|content| content.operations.iter()) {
        println!("{}", op);
        let ref ops = op.operands;
        match op.operator.as_str() {
            "m" => { // move x y
                ops_p!(ops, p => {
                    path.move_to(tr(p));
                    last = p;
                })
            }
            "l" => { // line x y
                ops_p!(ops, p => {
                    path.line_to(tr(p));
                    last = p;
                })
            }
            "c" => { // cubic bezier c1.x c1.y c2.x c2.y p.x p.y
                ops_p!(ops, c1, c2, p => {
                    path.bezier_curve_to(tr(c1), tr(c2), tr(p));
                    last = p;
                })
            }
            "v" => { // cubic bezier c2.x c2.y p.x p.y
                ops_p!(ops, c2, p => {
                    path.bezier_curve_to(tr(last), tr(c2), tr(p));
                    last = p;
                })
            }
            "y" => { // cubic c1.x c1.y p.x p.y
                ops_p!(ops, c1, p => {
                    path.bezier_curve_to(tr(c1), tr(p), tr(p));
                    last = p;
                })
            }
            "h" => { // close
                path.close_path();
            }
            "re" => { // rect x y width height
                ops_p!(ops, origin, size => {
                    let r = RectF::new(tr(origin), size);
                    dbg!(r);
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
                ops!(ops, a, b, c, d, e, f => {
                    let tr = canvas.current_transform().post_mul(
                        &Transform2DF::row_major(a, b, c, d, e, f)
                    );
                    canvas.set_current_transform(&tr);
                })
            }
            "w" => { // line width
                ops!(ops, width => {
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
            "sc" => { // fill color
                ops!(ops, r, g, b => {
                    canvas.set_fill_style(rgb2fill(r, g, b));
                });
            }
            "SC" => { // stroke color
                ops!(ops, r, g, b => {
                    canvas.set_stroke_style(rgb2fill(r, g, b));
                });
            }
            _ => {}
        }
    }
    
    canvas.into_scene()
}

fn main() {
    let path = args().nth(1).expect("no file given");
    println!("read: {}", path);
    let now = SystemTime::now();
    let file = run!(PdfFile::<Vec<u8>>::open(&path));
    
    let num_pages = file.get_root().pages.count;
    let mut pages = file.pages();
    for i in 0..num_pages {
        let p = file.get_page(i).unwrap();
        let mut out = fs::File::create(format!("{}_{}.svg", path, i)).expect("can't create output file");
        render_page(p).write_svg(&mut out);
    }
}
