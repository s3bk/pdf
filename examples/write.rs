extern crate pdf;

use std::env::args;
use std::time::SystemTime;
use std::fs;
use std::io::Write;

use pdf::file::File;
use pdf::print_err;
use pdf::object::*;
use pdf::content::*;
use pdf::primitive::Primitive;

fn main() {
    let mut f = File::new(Vec::new());
