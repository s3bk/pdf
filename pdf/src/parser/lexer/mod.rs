/// Lexing an input file, in the sense of breaking it up into substrings based on delimiters and
/// whitespace.

use std;
use std::str::FromStr;
use std::ops::Range;
use std::io::SeekFrom;

use error::*;

mod str;
pub use self::str::StringLexer;


/// `Lexer` has functionality to jump around and traverse the PDF lexemes of a string in any direction.
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct Lexer<'a> {
    pos: usize,
    buf: &'a [u8],
}

impl<'a> Lexer<'a> {
    pub fn new(buf: &'a [u8]) -> Lexer<'a> {
        Lexer {
            pos: 0,
            buf: buf,
        }
    }

    /// Returns next lexeme. Lexer moves to the next byte after the lexeme. (needs to be tested)
    pub fn next(&mut self) -> Result<Substr<'a>> {
        let (lexeme, pos) = self.next_word(true)?;
        self.pos = pos;
        Ok(lexeme)
    }

    /// Gives previous lexeme. Lexer moves to the first byte of this lexeme. (needs to be tested)
    pub fn back(&mut self) -> Result<Substr<'a>> {
        let (lexeme, pos) = self.next_word(false)?;
        self.pos = pos;
        Ok(lexeme)
    }

    /// Look at the next lexeme. Will return empty substr if the next character is EOF.
    pub fn peek(&self) -> Result<Substr<'a>> {
        match self.next_word(true) {
            Ok((substr, _)) => Ok(substr),
            Err(PdfError::EOF) => Ok(self.new_substr(self.pos..self.pos)),
            Err(e) => Err(e),
        }

    }

    /// Returns previous lexeme without advancing position.
    pub fn peek_back(&self) -> Result<Substr<'a>> {
        Ok(self.next_word(false)?.0)
    }

    /// Returns `Ok` if the next lexeme matches `expected` - else `Err`.
    pub fn next_expect(&mut self, expected: &'static str) -> Result<()> {
        let word = self.next()?;
        if word.equals(expected.as_bytes()) {
            Ok(())
        } else {
            Err(PdfError::UnexpectedLexeme {pos: self.pos, lexeme: word.to_string(), expected: expected})
        }
    }


    /// Used by next, peek and back - returns substring and new position
    /// If forward, places pointer at the next non-whitespace character.
    /// If backward, places pointer at the start of the current word.
    // TODO ^ backward case is actually not tested or.. thought about that well.
    fn next_word(&self, forward: bool) -> Result<(Substr<'a>, usize)> {
        let mut pos = self.pos;
        
        // Move away from eventual whitespace
        while self.is_whitespace(pos) {
            pos = self.advance_pos(pos, forward)?;
        }
        
        while self.buf[pos] == b'%' {
            if let Some(off) = self.buf[pos+1..].iter().position(|&b| b == b'\n') {
                pos += off+2;
            }
            
            // Move away from eventual whitespace
            while self.is_whitespace(pos) {
                pos = self.advance_pos(pos, forward)?;
            }
        }
        
        let start_pos = pos;

        // If first character is delimiter, this lexeme only contains that character.
        //  - except << and >> which go together
        if self.is_delimiter(pos) {
            // TODO +- 1
            if self.buf[pos] == b'<' && self.buf[pos+1] == b'<'
                || self.buf[pos] == b'>' && self.buf[pos+1] == b'>' {
                pos = self.advance_pos(pos, forward)?;

            }
            pos = self.advance_pos(pos, forward)?;
            return Ok((self.new_substr(start_pos..pos), pos));
        }

        // Read to past the end of lexeme
        while !self.is_whitespace(pos) && !self.is_delimiter(pos) {
            let new_pos = self.advance_pos(pos, forward)?;
            if new_pos == pos {
                break;
            } else {
                pos = new_pos;
            }
        }

        let result = self.new_substr(start_pos..pos);

        // Move away from whitespace again
        while self.is_whitespace(pos) {
            pos = self.advance_pos(pos, forward)?;
        }
        Ok((result, pos))
    }

    /// Just a helper for next_word.
    fn advance_pos(&self, pos: usize, forward: bool) -> Result<usize> {
        if forward {
            if pos < self.buf.len() {
                Ok(pos + 1)
            } else {
                Err(PdfError::EOF)
            }
        } else if pos > 0 {
            Ok(pos - 1)
        } else {
            Err(PdfError::EOF)
        }
    }

    pub fn next_as<T>(&mut self) -> Result<T>
        where T: FromStr, T::Err: std::error::Error + 'static
    {
        self.next().and_then(|word| word.to::<T>())
    }

    pub fn get_pos(&self) -> usize {
        self.pos
    }

    pub fn new_substr(&self, mut range: Range<usize>) -> Substr<'a> {
        // if the range is backward, fix it
        // start is inclusive, end is exclusive. keep that in mind
        if range.start > range.end {
            let new_end = range.start + 1;
            range.start = range.end + 1;
            range.end = new_end;
        }

        Substr {
            slice: &self.buf[range],
        }
    }


    /// Just a helper function for set_pos, set_pos_from_end and offset_pos.
    fn seek(&mut self, new_pos: SeekFrom) -> Substr<'a> {
        let wanted_pos;
        match new_pos {
            SeekFrom::Start(offset) => wanted_pos = offset as usize,
            SeekFrom::End(offset) => wanted_pos = self.buf.len() - offset as usize - 1,
            SeekFrom::Current(offset) => wanted_pos = self.pos + offset as usize,
        }

        let range = if self.pos < wanted_pos {
            self.pos..wanted_pos
        } else {
            wanted_pos..self.pos
        };
        self.pos = wanted_pos; // TODO restrict
        self.new_substr(range)
    }

    /// Returns the substr between the old and new positions
    pub fn set_pos(&mut self, new_pos: usize) -> Substr<'a> {
        self.seek(SeekFrom::Start(new_pos as u64))
    }
    /// Returns the substr between the old and new positions
    pub fn set_pos_from_end(&mut self, new_pos: usize) -> Substr<'a> {
        self.seek(SeekFrom::End(new_pos as i64))
    }
    /// Returns the substr between the old and new positions
    pub fn offset_pos(&mut self, offset: usize) -> Substr<'a> {
        self.seek(SeekFrom::Current(offset as i64))
    }

    /// Moves pos to start of next line. Returns the skipped-over substring.
    #[allow(dead_code)]
    pub fn seek_newline(&mut self) -> Substr{
        let start = self.pos;
        while self.buf[self.pos] != b'\n' 
            && self.incr_pos() { }
        self.incr_pos();

        self.new_substr(start..self.pos)
    }


    // TODO: seek_substr and seek_substr_back should use next() or back()?
    /// Moves pos to after the found `substr`. Returns Substr with traversed text if `substr` is found.
    #[allow(dead_code)]
    pub fn seek_substr(&mut self, substr: &[u8]) -> Option<Substr<'a>> {
        //
        let start = self.pos;
        let mut matched = 0;
        loop {
            if self.buf[self.pos] == substr[matched] {
                matched += 1;
            } else {
                matched = 0;
            }
            if matched == substr.len() {
                break;
            }
            if self.pos >= self.buf.len() {
                return None
            }
            self.pos += 1;
        }
        self.pos += 1;
        Some(self.new_substr(start..(self.pos - substr.len())))
    }


    //TODO perhaps seek_substr_back should, like back(), move to the first letter of the substr.
    /// Searches for string backward. Moves to after the found `substr`, returns the traversed
    /// Substr if found.
    pub fn seek_substr_back(&mut self, substr: &[u8]) -> Result<Substr<'a>> {
        let start = self.pos;
        let mut matched = substr.len();
        loop {
            if self.buf[self.pos] == substr[matched - 1] {
                matched -= 1;
            } else {
                matched = substr.len();
            }
            if matched == 0 {
                break;
            }
            if self.pos == 0 {
                err!(PdfError::NotFound {word: String::from(std::str::from_utf8(substr).unwrap())});
            }
            self.pos -= 1;
        }
        self.pos += substr.len();
        Ok(self.new_substr(self.pos..start))
    }

    /// Read and return slice of at most n bytes.
    #[allow(dead_code)]
    pub fn read_n(&mut self, n: usize) -> Substr<'a> {
        let start_pos = self.pos;
        self.pos += n;
        if self.pos >= self.buf.len() {
            self.pos = self.buf.len() - 1;
        }
        if start_pos < self.buf.len() {
            self.new_substr(start_pos..self.pos)
        } else {
            self.new_substr(0..0)
        }
    }

    /// Returns slice from current position to end.
    pub fn get_remaining_slice(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    fn incr_pos(&mut self) -> bool {
        if self.pos >= self.buf.len() - 1 {
            false
        } else {
            self.pos += 1;
            true
        }
    }
    fn is_whitespace(&self, pos: usize) -> bool {
        if pos >= self.buf.len() {
            false
        } else {
            self.buf[pos] == b' ' ||
            self.buf[pos] == b'\r' ||
            self.buf[pos] == b'\n' ||
            self.buf[pos] == b'\t'
        }
    }

    fn is_delimiter(&self, pos: usize) -> bool {
        self.buf.get(pos).map(|b| b"()<>[]{}/%".contains(&b)).unwrap_or(false)
    }
}



/// A slice from some original string - a lexeme.
pub struct Substr<'a> {
    slice: &'a [u8],
}
impl<'a> Substr<'a> {
    // to: &S -> U. Possibly expensive conversion.
    // as: &S -> &U. Cheap borrow conversion
    // into: S -> U. Cheap ownership transfer conversion.

    pub fn to_string(&self) -> String {
        String::from(self.as_str())
    }
    pub fn to_vec(&self) -> Vec<u8> {
        self.slice.to_vec()
    }
    pub fn to<T>(&self) -> Result<T>
        where T: FromStr, T::Err: std::error::Error + 'static
    {
        std::str::from_utf8(self.slice)?.parse::<T>().map_err(|e| PdfError::Parse { source: e.into() })
    }
    pub fn is_integer(&self) -> bool {
        match self.to::<i32>() {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    pub fn is_real_number(&self) -> bool {
        match self.to::<f32>() {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    
    pub fn as_str(&self) -> &str {
        // TODO use from_utf8_lossy - it's safe
        unsafe {
            std::str::from_utf8_unchecked(self.slice)
        }
    }
    pub fn as_slice(&self) -> &'a [u8] {
        self.slice
    }

    pub fn equals(&self, other: &[u8]) -> bool {
        self.slice == other
    }
}
