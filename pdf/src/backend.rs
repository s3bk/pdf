use memmap::Mmap;
use crate::error::*;
use crate::parser::Lexer;
use crate::parser::{read_xref_and_trailer_at};
use crate::xref::{XRefTable};
use crate::primitive::{Dictionary};
use crate::object::*;

use std::ops::{
    RangeFull,
    RangeFrom,
    RangeTo,
    Range,
};


pub trait Backend: Sized {
    fn read<T: IndexRange>(&self, range: T) -> Result<&[u8]>;
    fn write<T: IndexRange>(&mut self, range: T) -> Result<&mut [u8]>;
    fn len(&self) -> usize;

    /// Returns the value of startxref (currently only used internally!)
    fn locate_xref_offset(&self) -> Result<usize> {
        // locate the xref offset at the end of the file
        // `\nPOS\n%%EOF` where POS is the position encoded as base 10 integer.
        // u64::MAX has 20 digits + \n\n(2) + %%EOF(5) = 27 bytes max.

        let mut lexer = Lexer::new(self.read(..)?);
        lexer.set_pos_from_end(0);
        lexer.seek_substr_back(b"startxref")?;
        Ok(lexer.next()?.to::<usize>()?)
    }
    /// Used internally by File, but could also be useful for applications that want to look at the raw PDF objects.
    fn read_xref_table_and_trailer(&self) -> Result<(XRefTable, Dictionary)> {
        let xref_offset = self.locate_xref_offset()?;
        let mut lexer = Lexer::new(self.read(xref_offset..)?);
        
        let (xref_sections, trailer) = read_xref_and_trailer_at(&mut lexer, &NoResolve)?;
        
        let highest_id = trailer.get("Size")
            .ok_or_else(|| PdfError::MissingEntry {field: "Size".into(), typ: "XRefTable"})?
            .clone().as_integer()?;

        let mut refs = XRefTable::new(highest_id as ObjNr);
        for section in xref_sections {
            refs.add_entries_from(section);
        }
        
        let mut prev_trailer = {
            match trailer.get("Prev") {
                Some(p) => Some(p.as_integer()?),
                None => None
            }
        };
        println!("READ XREF AND TABLE");
        while let Some(prev_xref_offset) = prev_trailer {
            let mut lexer = Lexer::new(self.read(prev_xref_offset as usize..)?);
            let (xref_sections, trailer) = read_xref_and_trailer_at(&mut lexer, &NoResolve)?;
            
            for section in xref_sections {
                refs.add_entries_from(section);
            }
            
            prev_trailer = {
                match trailer.get("Prev") {
                    Some(p) => Some(p.as_integer()?),
                    None => None
                }
            };
        }
        Ok((refs, trailer))
    }
}


impl Backend for Mmap {
    fn read<T: IndexRange>(&self, range: T) -> Result<&[u8]> {
        let r = range.to_range(self.len());
        Ok(unsafe {
            &self.as_slice()[r]
        })
    }
    fn write<T: IndexRange>(&mut self, range: T) -> Result<&mut [u8]> {
        let r = range.to_range(self.len());
        Ok(unsafe {
            &mut self.as_mut_slice()[r]
        })
    }
    fn len(&self) -> usize {
        self.len()
    }
}


impl Backend for Vec<u8> {
    fn read<T: IndexRange>(&self, range: T) -> Result<&[u8]> {
        let r = range.to_range(self.len());
        Ok(&self[r])
    }
    fn write<T: IndexRange>(&mut self, range: T) -> Result<&mut [u8]> {
        let r = range.to_range(self.len());
        Ok(&mut self[r])
    }
    fn len(&self) -> usize {
        self.len()
    }
}



/// `IndexRange` is implemented by Rust's built-in range types, produced
/// by range syntax like `..`, `a..`, `..b` or `c..d`.
pub trait IndexRange
{
    #[inline]
    /// Start index (inclusive)
    fn start(&self) -> Option<usize> { None }
    #[inline]
    /// End index (exclusive)
    fn end(&self) -> Option<usize> { None }

    /// `len`: the size of whatever container that is being indexed
    fn to_range(&self, len: usize) -> Range<usize> {
        self.start().unwrap_or(0) .. self.end().unwrap_or(len)
    }
}


impl IndexRange for RangeFull {}

impl IndexRange for RangeFrom<usize> {
    #[inline]
    fn start(&self) -> Option<usize> { Some(self.start) }
}

impl IndexRange for RangeTo<usize> {
    #[inline]
    fn end(&self) -> Option<usize> { Some(self.end) }
}

impl IndexRange for Range<usize> {
    #[inline]
    fn start(&self) -> Option<usize> { Some(self.start) }
    #[inline]
    fn end(&self) -> Option<usize> { Some(self.end) }
}
