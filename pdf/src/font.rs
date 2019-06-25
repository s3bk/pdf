use object::*;
use primitive::*;
use error::*;
use std::io;

bitflags! {
    struct Flags: u32 {
        const FixedPitch    = 1 << 0;
        const Serif         = 1 << 1;
        const Symbolic      = 1 << 2;
        const Script        = 1 << 3;
        const Nonsymbolic   = 1 << 5;
        const Italic        = 1 << 6;
        const AllCap        = 1 << 16;
        const SmallCap      = 1 << 17;
        const ForceBold     = 1 << 18;
    }
}
/*
fn decode(flags: Flags, byte: u8) -> char {
    if flags.contains(Flags::Nonsymbolic) {
        // Adobe standard latin
        
    }
    if flags.contains(Flags::Symbolic) {
*/

#[derive(Object, Debug, Copy, Clone)]
pub enum FontType {
    Type0,
    Type1,
    MMType1,
    Type3,
    TrueType,
    CIDFontType0,
    CIDFontType2,
}

#[derive(Debug)]
pub struct Font {
    pub subtype: FontType,
    pub name: String,
    pub info: Option<TFont>
}
static STANDARD_FOTNS: &[(&'static str, &'static str)] = &[
    ("Courier", "CourierStd.otf"),
    ("Courier-Bold", "CourierStd-Bold.otf"),
    ("Courier-Oblique", "CourierStd-Oblique.otf"),
    ("Courier-BoldOblique", "CourierStd-BoldOblique.otf"),
    
    ("Times-Roman", "MinionPro-Regular.otf"),
    ("Times-Bold", "MinionPro-Bold.otf"),
    ("Times-Italic", "MinionPro-It.otf"),
    ("Times-BoldItalic", "MinionPro-BoldIt.otf"),
    
    ("Helvetica", "MyriadPro-Regular.otf"),
    ("Helvetica-Bold", "MyriadPro-Bold.otf"),
    ("Helvetica-Oblique", "MyriadPro-It.otf"),
    ("Helvetica-BoldOblique", "MyriadPro-BoldIt.otf"),
    
    ("Symbol", "SY______.PFB"),
    ("ZapfDingbats", "AdobePiStd.otf")
];

impl Object for Font {
    fn serialize<W: io::Write>(&self, _out: &mut W) -> Result<()> {unimplemented!()}
    fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
        let mut dict = p.to_dictionary(resolve)?;
        dict.expect("Font", "Type", "Font", true)?;
        let base_font = dict.require("Font", "BaseFont")?.to_name()?;
        let subtype = FontType::from_primitive(dict.require("Font", "Subtype")?, resolve)?;
        let mut info = match STANDARD_FOTNS.iter().filter(|&(name, filename)| *name == base_font).next() {
            Some(&(base_font, filename)) => None,
            None => {
                // reconstruct p
                let p = Primitive::Dictionary(dict);
                match subtype {
                    FontType::Type1 => Some(TFont::from_primitive(p, resolve)?),
                    FontType::TrueType => Some(TFont::from_primitive(p, resolve)?),
                    _ => None
                }
            }
        };
        
        Ok(Font {
            subtype,
            name: base_font,
            info
        })
    }
}
impl Font {
    pub fn data(&self) -> Option<Result<&[u8]>> {
        self.info.as_ref().and_then(|i| {
            if let Some(s) = i.font_descriptor.font_file3.as_ref() {
                return Some(s.data());
            }
            match self.subtype {
                FontType::Type1 => i.font_descriptor.font_file.as_ref().map(|s| s.data()),
                FontType::TrueType => i.font_descriptor.font_file2.as_ref().map(|s| s.data()),
                _ => None
            }
        })
    }
}
#[derive(Object, Debug)]
pub struct TFont {
    #[pdf(key="Name")]
    name: Option<String>,
    
    #[pdf(key="FirstChar")]
    first_char: i32,
    
    #[pdf(key="LastChar")]
    last_char: i32,
    
    #[pdf(key="Widths")]
    widths: Vec<f32>,
    
    #[pdf(key="FontDescriptor")]
    font_descriptor: FontDescriptor,
    
    #[pdf(key="Encoding")]
    encoding: Primitive,
    
    #[pdf(key="ToUnicode")]
    to_unicode: Option<Stream>
}

#[derive(Object, Debug)]
pub struct FontDescriptor {
    #[pdf(key="FontName")]
    font_name: String,
    
    #[pdf(key="FontFamily")]
    font_family: Option<String>,
    
    #[pdf(key="FontStretch")]
    font_stretch: Option<FontStretch>,

    #[pdf(key="FontWeight")]
    font_weight: Option<f32>,
    
    #[pdf(key="Flags")]
    flags: i32,
    
    #[pdf(key="FontBBox")]
    font_bbox: Rect,
    
    #[pdf(key="ItalicAngle")]
    italic_angle: f32,
    
    #[pdf(key="Ascent")]
    ascent: f32,
    
    #[pdf(key="Descent")]
    descent: f32,
    
    #[pdf(key="Leading", default="0.")]
    leading: f32,
    
    #[pdf(key="CapHeight")]
    cap_height: f32,
    
    #[pdf(key="XHeight", default="0.")]
    xheight: f32,
    
    #[pdf(key="StemV", default="0.")]
    stem_v: f32,
    
    #[pdf(key="StemH", default="0.")]
    stem_h: f32,
    
    #[pdf(key="AvgWidth", default="0.")]
    avg_width: f32,
    
    #[pdf(key="MaxWidth", default="0.")]
    max_width: f32,
    
    #[pdf(key="MissingWidth", default="0.")]
    missing_width: f32,
    
    #[pdf(key="FontFile")]
    font_file: Option<Stream>,
    
    #[pdf(key="FontFile2")]
    font_file2: Option<Stream>,
    
    #[pdf(key="FontFile3")]
    font_file3: Option<Stream<FontStream3>>,
    
    #[pdf(key="CharSet")]
    char_set: Option<PdfString>
}

#[derive(Object, Debug, Clone)]
#[pdf(key="Subtype")]
enum FontTypeExt {
    Type1C,
    CIDFontType0C,
    OpenType
}
#[derive(Object, Debug, Clone)]
struct FontStream3 {
    #[pdf(key="Subtype")]
    subtype: FontTypeExt
}

#[derive(Object, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded
}
