extern crate pdf;

use std::env::args;
use std::time::SystemTime;
use std::fs;
use std::io::Write;
use std::error::Error;
use pdf::file::File;
use pdf::object::*;
use pdf::content::*;
use pdf::primitive::Primitive;

fn add_primitive(p: &Primitive, out: &mut String) {
    // println!("p: {:?}", p);
    match p {
        &Primitive::String(ref s) => if let Ok(text) = s.as_str() {
            out.push_str(text);
        }
        &Primitive::Array(ref a) => for p in a.iter() {
            add_primitive(p, out);
        }
        _ => ()
    }
}

fn main() -> Result<(), Box<Error>> {
    let path = args().nth(1).expect("no file given");
    println!("read: {}", path);
    let now = SystemTime::now();
    let file = File::<Vec<u8>>::open(&path)?;
    
    let mut out = String::new();
    for page in file.pages() {
        for content in &page.contents {
            for &Operation { ref operator, ref operands } in &content.operations {
                // println!("{} {:?}", operator, operands);
                match operator.as_str() {
                    "Tj" | "TJ" | "BT" => operands.iter().for_each(|p| add_primitive(p, &mut out)),
                    _ => {}
                }
            }
        }
    }
    println!("{}", out);
    
    Ok(())
}
