//! Models of PDF types

use std::io;
use object::*;
use error::*;
use content::Content;
use font::Font;
use file::File;
use backend::Backend;
use std::rc::Rc;
use std::ops::Deref;

/// Node in a page tree - type is either `Page` or `PageTree`
#[derive(Debug)]
pub enum PagesNode {
    Tree (PageTree),
    Leaf (Page),
}
impl Object for PagesNode {
    fn serialize<W: io::Write>(&self, out: &mut W) -> Result<()> {
        match *self {
            PagesNode::Tree (ref t) => t.serialize(out),
            PagesNode::Leaf (ref l) => l.serialize(out),
        }
    }
    fn from_primitive(p: Primitive, r: &dyn Resolve) -> Result<PagesNode> {
        let dict = Dictionary::from_primitive(p, r)?;
        match dict["Type"].clone().to_name()?.as_str() {
            "Page" => Ok(PagesNode::Leaf (Page::from_primitive(Primitive::Dictionary(dict), r)?)),
            "Pages" => Ok(PagesNode::Tree (PageTree::from_primitive(Primitive::Dictionary(dict), r)?)),
            other => Err(PdfError::WrongDictionaryType {expected: "Page or Pages".into(), found: other.into()}),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageRc(pub Rc<PagesNode>);
impl Deref for PageRc {
    type Target = Page;
    fn deref(&self) -> &Page {
        match *self.0 {
            PagesNode::Leaf(ref page) => page,
            _ => panic!("PageRc that isn't a Page")
        }
    }
}


#[derive(Object, Default)]
pub struct Catalog {
// Version: Name,
    #[pdf(key="Pages")]
    pub pages: PageTree,
// PageLabels: number_tree,
    #[pdf(key="Names")]
    pub names: Option<NameDictionary>,
// Dests: Dict
// ViewerPreferences: dict
// PageLayout: name
// PageMode: name
// Outlines: dict
// Threads: array
// OpenAction: array or dict
// AA: dict
// URI: dict
// AcroForm: dict
// Metadata: stream
    #[pdf(key="StructTreeRoot")]
    pub struct_tree_root: Option<StructTreeRoot>,
// MarkInfo: dict
// Lang: text string
// SpiderInfo: dict
// OutputIntents: array
// PieceInfo: dict
// OCProperties: dict
// Perms: dict
// Legal: dict
// Requirements: array
// Collection: dict
// NeedsRendering: bool
}


#[derive(Object, Debug, Default)]
#[pdf(Type = "Pages")]
pub struct PageTree {
    #[pdf(key="Parent")]
    pub parent: Option<Ref<PageTree>>,
    #[pdf(key="Kids")]
    pub kids:   Vec<Ref<PagesNode>>,
    #[pdf(key="Count")]
    pub count:  i32,


    /// Exists to be inherited to a 'Page' object. Note: *Inheritable*.
    // Note about inheritance..= if we wanted to 'inherit' things at the time of reading, we would
    // want Option<Ref<Resources>> here most likely.
    #[pdf(key="Resources")]
    pub resources: Option<Ref<Resources>>,
    
    #[pdf(key="MediaBox")]
    pub media_box:  Option<Rect>,
    
    #[pdf(key="CropBox")]
    pub crop_box:   Option<Rect>,
}

#[derive(Object, Debug)]
pub struct Page {
    #[pdf(key="Parent")]
    pub parent: Ref<PageTree>,

    #[pdf(key="Resources")]
    pub resources: Option<Ref<Resources>>,
    
    #[pdf(key="MediaBox")]
    pub media_box:  Option<Rect>,
    
    #[pdf(key="CropBox")]
    pub crop_box:   Option<Rect>,
    
    #[pdf(key="TrimBox")]
    pub trim_box:   Option<Rect>,
    
    #[pdf(key="Contents")]
    pub contents:   Option<Content>
}
fn inherit<T, F, B: Backend>(mut parent: Ref<PageTree>, file: &File<B>, f: F) -> Result<Option<T>>
    where F: Fn(Rc<PageTree>) -> Option<Result<T>>
{
    loop {
        let page_tree = file.deref(parent)?;
        
        match (page_tree.parent, f(page_tree)) {
            (_, Some(t)) => break Ok(Some(t?)),
            (Some(p), None) => parent = p,
            (None, None) => break Ok(None)
        }
    }
}

impl Page {
    pub fn new(parent: Ref<PageTree>) -> Page {
        Page {
            parent:     parent,
            media_box:  None,
            crop_box:   None,
            trim_box:   None,
            resources:  None,
            contents:   None
        }
    }
    pub fn media_box<B: Backend>(&self, file: &File<B>) -> Result<Rect> {
        match self.media_box {
            Some(b) => Ok(b),
            None => inherit(self.parent, file, |pt| pt.media_box.map(|b| Ok(b)))?
                .ok_or_else(|| PdfError::MissingEntry { typ: "Page", field: "MediaBox".into() })
        }
    }
    pub fn crop_box<B: Backend>(&self, file: &File<B>) -> Result<Rect> {
        match self.crop_box {
            Some(b) => Ok(b),
            None => match inherit(self.parent, file, |pt| pt.crop_box.map(|b| Ok(b)))? {
                Some(b) => Ok(b),
                None => self.media_box(file)
            }
        }
    }
    pub fn resources<B: Backend>(&self, file: &File<B>) -> Result<Rc<Resources>> {
        match self.resources {
            Some(r) => file.deref(r),
            None => inherit(self.parent, file, |pt| pt.resources.map(|r| file.deref(r)))?
                .ok_or_else(|| PdfError::MissingEntry { typ: "Page", field: "Resources".into() })
        }
    }
}

#[derive(Object)]
pub struct PageLabel {
    #[pdf(key="S")]
    style:  Option<Counter>,
    
    #[pdf(key="P")]
    prefix: Option<PdfString>,
    
    #[pdf(key="St")]
    start:  Option<usize>
}

#[derive(Object, Debug)]
pub struct Resources {
    #[pdf(key="ExtGState")]
    pub ext_g_state: Option<GraphicsStateParameters>,
    // color_space: Option<ColorSpace>,
    // pattern: Option<Pattern>,
    // shading: Option<Shading>,
    #[pdf(key="XObject")]
    pub xobjects: Option<BTreeMap<String, XObject>>,
    // /XObject is a dictionary that map arbitrary names to XObjects
    #[pdf(key="Font")]
    pub fonts: Option<BTreeMap<String, Font>>,
}
impl Resources {
    pub fn fonts(&self) -> impl Iterator<Item=(&str, &Font)> {
        self.fonts.iter().flat_map(|b| b.iter()).map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Object, Debug)]
#[pdf(Type = "ExtGState?")]
/// `ExtGState`
pub struct GraphicsStateParameters {
    //TODO
}

#[derive(Object, Debug)]
#[pdf(is_stream)]
pub enum XObject {
    #[pdf(name="PS")]
    Postscript (PostScriptXObject),
    Image (ImageXObject),
    Form (FormXObject),
}

/// A variant of XObject
pub type PostScriptXObject = Stream<PostScriptDict>;
/// A variant of XObject
pub type ImageXObject = Stream<ImageDict>;
/// A variant of XObject
pub type FormXObject = Stream<FormDict>;

#[derive(Object, Debug)]
#[pdf(Type="XObject", Subtype="PS")]
pub struct PostScriptDict {
    // TODO
}

#[derive(Object, Debug)]
#[pdf(Type="XObject", Subtype="Image")]
/// A variant of XObject
pub struct ImageDict {
    #[pdf(key="Width")]
    pub width: i32,
    #[pdf(key="Height")]
    pub height: i32,
    // ColorSpace: name or array
    #[pdf(key="BitsPerComponent")]
    pub bits_per_component: i32,
    // Note: only allowed values are 1, 2, 4, 8, 16. Enum?
    
    #[pdf(key="Intent")]
    pub intent: Option<RenderingIntent>,
    // Note: default: "the current rendering intent in the graphics state" - I don't think this
    // ought to have a default then

    #[pdf(key="ImageMask", default="false")]
    pub image_mask: bool,

    // Mask: stream or array
    //
    /// Describes how to map image samples into the range of values appropriate for the image’s color space.
    /// If `image_mask`: either [0 1] or [1 0]. Else, the length must be twice the number of color
    /// components required by `color_space` (key ColorSpace)
    // (see Decode arrays page 344)
    #[pdf(key="Decode")]
    pub decode: Vec<i32>,

    #[pdf(key="Interpolate", default="false")]
    pub interpolate: bool,

    // Alternates: Vec<AlternateImage>

    // SMask (soft mask): stream
    // SMaskInData: i32
    ///The integer key of the image’s entry in the structural parent tree
    #[pdf(key="StructParent")]
    pub struct_parent: Option<i32>,

    #[pdf(key="ID")]
    pub id: Option<PdfString>,

    // OPI: dict
    // Metadata: stream
    // OC: dict
    
}


#[derive(Object, Debug, Clone)]
pub enum RenderingIntent {
    AbsoluteColorimetric,
    RelativeColorimetric,
    Saturation,
    Perceptual,
}


#[derive(Object, Debug)]
#[pdf(Type="XObject", Subtype="Form")]
pub struct FormDict {
    // TODO
}


pub enum Counter {
    Arabic,
    RomanUpper,
    RomanLower,
    AlphaUpper,
    AlphaLower
}
impl Object for Counter {
    fn serialize<W: io::Write>(&self, out: &mut W) -> Result<()> {
        let style_code = match *self {
            Counter::Arabic     => "D",
            Counter::RomanLower => "r",
            Counter::RomanUpper => "R",
            Counter::AlphaLower => "a",
            Counter::AlphaUpper => "A"
        };
        out.write_all(style_code.as_bytes())?;
        Ok(())
    }
    fn from_primitive(_: Primitive, _: &dyn Resolve) -> Result<Self> {
        unimplemented!();
    }
}



pub enum NameTreeNode<T> {
    ///
    Intermediate (Vec<Ref<NameTree<T>>>),
    ///
    Leaf (Vec<(PdfString, T)>)

}
/// Note: The PDF concept of 'root' node is an intermediate or leaf node which has no 'Limits'
/// entry. Hence, `limits`
pub struct NameTree<T> {
    limits: Option<(PdfString, PdfString)>,
    node: NameTreeNode<T>,
}

impl<T: Object> Object for NameTree<T> {
    fn serialize<W: io::Write>(&self, _out: &mut W) -> Result<()> {
        unimplemented!();
    }
    fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
        let mut dict = p.to_dictionary(resolve)?;
        
        // Quite long function..=
        let limits = match dict.remove("Limits") {
            Some(limits) => {
                let limits = limits.to_array(resolve)?;
                if limits.len() != 2 {
                    bail!("Error reading NameTree: 'Limits' is not of length 2");
                }
                let min = limits[0].clone().to_string()?;
                let max = limits[1].clone().to_string()?;

                Some((min, max))
            }
            None => None

        };

        let kids = dict.remove("Kids");
        let names = dict.remove("Names");
        // If no `kids`, try `names`. Else there is an error.
        Ok(match kids {
            Some(kids) => {
                let kids = kids.to_array(resolve)?.iter().map(|kid|
                    Ref::<NameTree<T>>::from_primitive(kid.clone(), resolve)
                ).collect::<Result<Vec<_>>>()?;
                NameTree {
                    limits: limits,
                    node: NameTreeNode::Intermediate (kids)
                }
            }

            None =>
                match names {
                    Some(names) => {
                        let names = names.to_array(resolve)?;
                        let mut new_names = Vec::new();
                        for pair in names.chunks(2) {
                            let name = pair[0].clone().to_string()?;
                            let value = T::from_primitive(pair[1].clone(), resolve)?;
                            new_names.push((name, value));
                        }
                        NameTree {
                            limits: limits,
                            node: NameTreeNode::Leaf (new_names),
                        }
                    }
                    None => bail!("Neither Kids nor Names present in NameTree node.")
                }
        })
    }
}




/// There is one `NameDictionary` associated with each PDF file.
#[derive(Object)]
pub struct NameDictionary {
    /*
    #[pdf(key="Dests")]
    ap: NameTree<T>,
    #[pdf(key="AP")]
    ap: NameTree<T>,
    #[pdf(key="JavaScript")]
    javascript: NameTree<T>,
    #[pdf(key="Pages")]
    pages: NameTree<T>,
    #[pdf(key="Templates")]
    templates: NameTree<T>,
    #[pdf(key="IDS")]
    ids: NameTree<T>,
    #[pdf(key="URLS")]
    urls: NameTree<T>,
    */
    #[pdf(key="EmbeddedFiles")]
    embedded_files: Option<FileSpec>,
    /*
    #[pdf(key="AlternativePresentations")]
    alternate_presentations: NameTree<AlternatePresentation>,
    #[pdf(key="Renditions")]
    renditions: NameTree<Rendition>,
    */
}

/* Embedded file streams can be associated with the document as a whole through
 * the EmbeddedFiles entry (PDF 1.4) in the PDF document’s name dictionary
 * (see Section 3.6.3, “Name Dictionary”).
 * The associated name tree maps name strings to file specifications that refer
 * to embedded file streams through their EF entries.
*/

#[derive(Object, Debug, Clone)]
pub struct FileSpec {
    #[pdf(key="EF")]
    ef: Option<Files<EmbeddedFile>>,
    /*
    #[pdf(key="RF")]
    rf: Option<Files<RelatedFilesArray>>,
    */
}

/// Used only as elements in `FileSpec`
#[derive(Object, Debug, Clone)]
pub struct Files<T: Object> {
    #[pdf(key="F")]
    f: Option<T>,
    #[pdf(key="UF")]
    uf: Option<T>,
    #[pdf(key="DOS")]
    dos: Option<T>,
    #[pdf(key="Mac")]
    mac: Option<T>,
    #[pdf(key="Unix")]
    unix: Option<T>,
}

/// PDF Embedded File Stream.
#[derive(Object, Debug, Clone)]
pub struct EmbeddedFile {
    /*
    #[pdf(key="Subtype")]
    subtype: Option<String>,
    */
    #[pdf(key="Params")]
    params: Option<EmbeddedFileParamDict>,
}

#[derive(Object, Debug, Clone)]
pub struct EmbeddedFileParamDict {
    #[pdf(key="Size")]
    size: Option<i32>,
    /*
    // TODO need Date type
    #[pdf(key="CreationDate")]
    creationdate: T,
    #[pdf(key="ModDate")]
    moddate: T,
    #[pdf(key="Mac")]
    mac: T,
    #[pdf(key="CheckSum")]
    checksum: T,
    */
}






pub fn write_list<'a, W, T: 'a, I>(out: &mut W, mut iter: I) -> Result<()>
    where W: io::Write, T: Object, I: Iterator<Item=&'a T>
{
    write!(out, "[")?;
    
    if let Some(first) = iter.next() {
        first.serialize(out)?;
        
        for other in iter {
            out.write_all(b", ")?;
            other.serialize(out)?;
        }
    }
    
    write!(out, "]")?;
    Ok(())
}

#[derive(Object)]
pub struct Outlines {
    #[pdf(key="Count")]
    pub count:  usize
}

#[derive(Debug, Copy, Clone)]
pub struct Rect {
    pub left:   f32,
    pub bottom: f32,
    pub right:  f32,
    pub top:    f32,
}
impl Object for Rect {
    fn serialize<W: io::Write>(&self, out: &mut W) -> Result<()> {
        write!(out, "[{} {} {} {}]", self.left, self.top, self.right, self.bottom)?;
        Ok(())
    }
    fn from_primitive(p: Primitive, r: &dyn Resolve) -> Result<Self> {
        let arr = p.to_array(r)?;
        if arr.len() != 4 {
            bail!("len != 4");
        }
        Ok(Rect {
            left:   arr[0].as_number()?,
            bottom: arr[1].as_number()?,
            right:  arr[2].as_number()?,
            top:    arr[3].as_number()?
        })
    }
}


// Stuff from chapter 10 of the PDF 1.7 ref

#[derive(Object)]
pub struct MarkInformation { // TODO no /Type
    /// indicating whether the document conforms to Tagged PDF conventions
    #[pdf(key="Marked", default="false")]
    pub marked: bool,
    /// Indicating the presence of structure elements that contain user properties attributes
    #[pdf(key="UserProperties", default="false")]
    pub user_properties: bool, 
    /// Indicating the presence of tag suspects
    #[pdf(key="Suspects", default="false")]
    pub suspects: bool,
}

#[derive(Object)]
#[pdf(Type = "StructTreeRoot")]
pub struct StructTreeRoot {
    #[pdf(key="K")]
    pub children: Vec<StructElem>,
}
#[derive(Object)]
pub struct StructElem {
    #[pdf(key="S")]
    /// `S`
    struct_type: StructType,
    #[pdf(key="P")]
    /// `P`
    parent: Ref<StructElem>,
    #[pdf(key="ID")]
    /// `ID`
    id: Option<PdfString>,
    #[pdf(key="Pg")]
    /// `Pg`: A page object representing a page on which some or all of the content items designated by the K entry are rendered.
    page: Option<Ref<Page>>,
}


#[derive(Object)]
pub enum StructType {
    Document,
    Part,
    Art,
    Sect,
    Div,
    BlockQuote,
    Caption,
    TOC,
    TOCI,
    Index,
    NonStruct,
    Private,
    Book,
}

