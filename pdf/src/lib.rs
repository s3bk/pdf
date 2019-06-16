#![feature(attr_literals)]
#![recursion_limit="128"]
//#![feature(collections_range)]
//#![feature(slice_get_slice)]
#![allow(non_camel_case_types)]  /* TODO temporary becaues of pdf_derive */
#![allow(unused_doc_comments)] // /* TODO temporary because of err.rs */
#![feature(use_extern_macros)] // because of error-chain experimenting

#[macro_use]
extern crate pdf_derive;
#[macro_use]
extern crate snafu;

extern crate num_traits;
extern crate inflate;
extern crate itertools;
extern crate memmap;
extern crate tuple;
extern crate chrono;

#[macro_use]
pub mod error;
//mod macros;
pub mod object;
pub mod xref;
pub mod primitive;
pub mod file;
pub mod backend;
pub mod content;
pub mod parser;

// mod content;
mod enc;

// pub use content::*;
use error::*;

// hack to use ::pdf::object::Object in the derive
mod pdf {
    pub use super::*;
}
