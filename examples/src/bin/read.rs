extern crate pdf;

use std::env::args;
use std::time::SystemTime;

use pdf::file::File;
use pdf::object::*;
use pdf::backend::Backend;

macro_rules! run {
    ($e:expr) => (
        match $e {
            Ok(r) => r,
            Err(e) => return e.trace()
        }
    )
}
fn walk_pagetree<B: Backend>(file: &File<B>, pos: &mut usize, tree: Ref<PagesNode>) {
    match *(file.deref(tree).unwrap()) {
        PagesNode::Tree(ref child) => {
            for &k in &child.kids {
                walk_pagetree(file, pos, k);
            }
        },
        PagesNode::Leaf(ref page) => {
            println!("{} {:?}", *pos, page);
            *pos += 1;
        }
    }
}
fn main() {
    let path = args().nth(1).expect("no file given");
    println!("read: {}", path);
    let now = SystemTime::now();
    let file = run!(File::<Vec<u8>>::open(&path));
    
    for &k in &file.get_root().pages.kids {
        walk_pagetree(&file, &mut 0, k);
    }
    
    let mut num_fonts = 0;
    let mut num_images = 0;
    file.pages(|_, page| {
        let resources = page.resources(&file).unwrap();
        for xobject in resources.xobjects.iter().flat_map(|d| d.values()) {
            if let XObject::Image(ref im) = xobject {
                num_images += 1;
            }
        }
        num_fonts += resources.fonts.iter().flat_map(|d| d.values()).count();
    });
    println!("Found {} image(s).", num_images);

    println!("Found {} font(s).", num_fonts);
    
    if let Ok(elapsed) = now.elapsed() {
        println!("Time: {}s", elapsed.as_secs() as f64
                 + elapsed.subsec_nanos() as f64 * 1e-9);
    }
}
