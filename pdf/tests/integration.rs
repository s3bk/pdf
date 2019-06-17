extern crate pdf;
extern crate memmap;
extern crate glob;

use std::str;
use memmap::Mmap;
use pdf::file::File;
use pdf::object::*;
use pdf::parser::parse;
use glob::glob;

macro_rules! file_path {
    ( $subdir:expr ) => { concat!("../files/", $subdir) }
}
macro_rules! run {
    ($e:expr) => (
        match $e {
            Ok(v) => v,
            Err(e) => {
                e.trace();
                panic!("{}", e);
            }
        }
    )
}

#[test]
fn open_file() {
    let _ = run!(File::<Vec<u8>>::open(file_path!("example.pdf")));
    let _ = run!(File::<Mmap>::open(file_path!("example.pdf")));
}

#[test]
fn read_pages() {
    for entry in glob(file_path!("*.pdf")).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                println!("\n\n == Now testing `{}` ==\n", path.to_str().unwrap());

                let path = path.to_str().unwrap();
                let file = run!(File::<Vec<u8>>::open(path));
                let num_pages = file.get_root().pages.count;
                for i in 0..num_pages {
                    println!("\nRead page {}", i);
                    let _ = file.get_page(i);
                }
            }
            Err(e) => println!("{:?}", e)
        }
    }
}

#[test]
fn parse_objects_from_stream() {
    use pdf::object::NO_RESOLVE;
    let file = run!(File::<Vec<u8>>::open(file_path!("xelatex.pdf")));
    // .. we know that object 13 of that file is an ObjectStream
    let obj_stream = run!(file.deref(Ref::<ObjectStream>::new(PlainRef {id: 13, gen: 0})));
    for i in 0..obj_stream.n_objects() {
        let slice = run!(obj_stream.get_object_slice(i));
        println!("Object slice #{}: {}\n", i, str::from_utf8(slice).unwrap());
        run!(parse(slice, NO_RESOLVE));
    }
}

// TODO test decoding
