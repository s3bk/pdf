use failure::Error;
use object::ObjNr;

#[derive(Debug, Fail)]
#[fail(display = "An error occurred.")]
pub enum PdfError {
    // Syntax / parsing
    #[fail(display="Unexpected end of file")]
    EOF,
    
    #[fail(display="Error parsing from string: {}", error)]
    Parse {
        #[fail(cause)]
        error: std::string::ParseError
    },
    
    #[fail(display="Unexpected token '{}' at {} - expected '{}'", lexeme, pos, expected)]
    UnexpectedLexeme {pos: usize, lexeme: String, expected: &'static str},
    
    #[fail(display="Expecting an object, encountered {} at pos {}. Rest:\n{}\n\n((end rest))", first_lexeme, pos, rest)]
    UnknownType {pos: usize, first_lexeme: String, rest: String},
    
    #[fail(display="Unknown variant '{}' for enum {}", name, id)]
    UnknownVariant { id: &'static str, name: String },
    
    #[fail(display="'{}' not found.", word)]
    NotFound { word: String },
    
    #[fail(display="Cannot follow reference during parsing - no resolve fn given (most likely /Length of Stream).")]
    Reference, // TODO: which one?
    
    #[fail(display="Erroneous 'type' field in xref stream - expected 0, 1 or 2, found {}", found)]
    XRefStreamType { found: u64 },
    
    #[fail(display="Parsing read past boundary of Contents.")]
    ContentReadPastBoundary,
    
    //////////////////
    // Encode/decode
    #[fail(display="Hex decode error. Position {}, bytes {:?}", pos, bytes)]
    HexDecode {pos: usize, bytes: [u8; 2]},
    
    #[fail(display="Ascii85 tail error")]
    Ascii85TailError,
    
    #[fail(display="Failed to convert '{}' into PredictorType", n)]
    IncorrectPredictorType {n: u8},
    
    //////////////////
    // Dictionary
    #[fail(display="Can't parse field {} of struct {} due to: {}", field, typ, error)]
    FromPrimitiveError {
        typ: &'static str,
        field: &'static str,
        #[fail(cause)]
        error: Box<PdfError>
    },
    
    #[fail(display="Field {} is missing in dictionary for type {}.", field, typ)]
    MissingEntry {
        typ: &'static str,
        field: &'static str
    },
    
    #[fail(display="Expected to find value {} for key {}. Found {} instead.", value, key, found)]
    KeyValueMismatch {
        key: &'static str,
        value: &'static str,
        found: String,
    },
    
    #[fail(display="Expected dictionary /Type = {}. Found /Type = {}.", expected, found)]
    WrongDictionaryType {expected: String, found: String},
    
    //////////////////
    // Misc
    #[fail(display="Tried to dereference free object nr {}.", obj_nr)]
    FreeObject {obj_nr: u64},
    
    #[fail(display="Tried to dereference non-existing object nr {}.", obj_nr)]
    NullRef {obj_nr: u64},

    #[fail(display="Expected primitive {}, found primive {} instead.", expected, found)]
    UnexpectedPrimitive {expected: &'static str, found: &'static str},
    /*
    WrongObjectType {expected: &'static str, found: &'static str} {
        description("Function called on object of wrong type.")
        display("Expected {}, found {}.", expected, found)
    }
    */
    #[fail(display="Object stream index out of bounds ({}/{}).", index, max)]
    ObjStmOutOfBounds {index: usize, max: usize},
    
    #[fail(display="Page out of bounds ({}/{}).", page_nr, max)]
    PageOutOfBounds {page_nr: i32, max: i32},
    
    #[fail(display="Page {} could not be found in the page tree.", page_nr)]
    PageNotFound {page_nr: i32},
    
    #[fail(display="Entry {} in xref table unspecified", id)]
    UnspecifiedXRefEntry {id: ObjNr},
    
    #[fail(display="{}", error)]
    Other { #[cause] error: Error },
    
    #[fail(display="{}", error)]
    OtherS { error: String }
}

pub type Result<T> = std::result::Result<T, PdfError>;

impl From<std::string::ParseError> for PdfError {
    fn from(error: std::string::ParseError) -> PdfError {
        PdfError::Parse { error }
    }
}
impl From<Error> for PdfError {
    fn from(error: Error) -> PdfError {
        PdfError::Other { error }
    }
}
impl From<String> for PdfError {
    fn from(error: String) -> PdfError {
        PdfError::OtherS { error }
    }
}

macro_rules! err {
    ($e: expr) => ({
        return Err($e);
    })
}
macro_rules! bail {
    ($($t:tt)*) => {
        err!(PdfError::OtherS { error: format!($($t)*) })
    }
}
