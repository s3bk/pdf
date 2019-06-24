#![allow(non_camel_case_types)]  /* TODO temporary becaues of pdf_derive */
#![allow(unused_doc_comments)] // /* TODO temporary because of err.rs */
#![feature(custom_attribute)]
#![feature(termination_trait_lib)]
#![feature(core_intrinsics)]

#[macro_use] extern crate pdf_derive;
#[macro_use] extern crate snafu;
#[macro_use] extern crate bitflags;
#[macro_use] extern crate log;

extern crate num_traits;
extern crate inflate;
extern crate itertools;
extern crate memmap;
extern crate tuple;
extern crate chrono;
extern crate once_cell;

#[macro_use] pub mod error;
//mod macros;
pub mod object;
pub mod xref;
pub mod primitive;
pub mod file;
pub mod backend;
pub mod content;
pub mod parser;
pub mod font;
pub mod any;

// mod content;
mod enc;

// pub use content::*;
pub use error::PdfError;

// hack to use ::pdf::object::Object in the derive
pub mod pdf {
    pub use super::*;
}
