extern crate pdf;

use std::env::args;
use std::time::SystemTime;
use std::collections::HashMap;

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
fn main() {
    let path = args().nth(1).expect("no file given");
    println!("read: {}", path);
    let now = SystemTime::now();
    let file = run!(File::<Vec<u8>>::open(&path));
    
    let mut resources = HashMap::new();
    let mut num_fonts = 0;
    let mut num_images = 0;
    for page in file.pages() {
        let r = page.unwrap().resources(&file).unwrap();
        for xobject in r.xobjects.iter().flat_map(|d| d.values()) {
            if let XObject::Image(ref im) = xobject {
                num_images += 1;
            }
        }
        resources.insert((&*r) as *const _, r);
    }
    println!("Found {} image(s).", num_images);

    let mut fonts = HashMap::new();
    for r in resources.values() {
        for (name, font) in r.fonts() {
            fonts.insert(font.name.as_str(), font);
        }
    }
    for font in fonts.values() {
        println!("{:?}", font);
    }
    if let Ok(elapsed) = now.elapsed() {
        println!("Time: {}s", elapsed.as_secs() as f64
                 + elapsed.subsec_nanos() as f64 * 1e-9);
    }
}
