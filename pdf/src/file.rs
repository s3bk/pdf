//! This is kind of the entry-point of the type-safe PDF functionality.
use std;
use std::io::Read;
use std::{str};
use std::marker::PhantomData;
use std::collections::HashMap;
use error::*;
use object::*;
use xref::XRefTable;
use primitive::{Primitive, Dictionary, PdfString};
use backend::Backend;
use std::rc::Rc;
use any::Any;
use std::cell::RefCell;
use std::ops::Range;

pub struct PromisedRef<T> {
    inner:      PlainRef,
    _marker:    PhantomData<T>
}
impl<'a, T> Into<PlainRef> for &'a PromisedRef<T> {
    fn into(self) -> PlainRef {
        self.inner
    }
}
impl<'a, T> Into<Ref<T>> for &'a PromisedRef<T> {
    fn into(self) -> Ref<T> {
        Ref::new(self.into())
    }
}


pub struct File<B: Backend> {
    backend:    B,
    trailer:    Trailer,
    refs:       XRefTable,
    changes:    HashMap<ObjNr, Primitive>,
    cache:      RefCell<HashMap<PlainRef, Any>>
}

impl<B: Backend> File<B> {
    pub fn new(b: B) -> File<B> {
        File {
            backend:    b,
            trailer:    Trailer::default(),
            refs:       XRefTable::new(1), // the root object,
            changes:    HashMap::new(),
            cache:      RefCell::new(HashMap::new())
        }
    }

    /// Opens the file at `path` and uses Vec<u8> as backend.
    pub fn open(path: &str) -> Result<File<Vec<u8>>> {
        // Read file contents to Vec
        let mut backend = Vec::new();
        let mut f = std::fs::File::open(path)?;
        f.read_to_end(&mut backend)?;

        let (refs, trailer) = backend.read_xref_table_and_trailer()?;
        let trailer = Trailer::from_primitive(Primitive::Dictionary(trailer), &|r| backend.resolve(&refs, r))?;
        
        Ok(File {
            backend:    backend,
            trailer:    trailer,
            refs:       refs,
            changes:    HashMap::new(),
            cache:      RefCell::new(HashMap::new())
        })
    }


    pub fn get_root(&self) -> &Catalog {
        &self.trailer.root
    }

    fn resolve(&self, r: PlainRef) -> Result<Primitive> {
        match self.changes.get(&r.id) {
            Some(ref p) => Ok((*p).clone()),
            None => self.backend.resolve(&self.refs, r)
        }
    }

    pub fn deref<T>(&self, r: Ref<T>) -> Result<Rc<T>>
        where T: Object + 'static
    {
        use std::collections::hash_map::Entry;
        match self.cache.borrow_mut().entry(r.get_inner()) {
            Entry::Occupied(e) => {
                Ok(e.get().clone().downcast().expect("wrong type"))
            },
            Entry::Vacant(mut e) => {
                let primitive = self.resolve(r.get_inner())?;
                let obj = T::from_primitive(primitive, &|id| self.resolve(id))?;
                let rc = Rc::new(obj);
                e.insert(Any::new(rc.clone()));
                Ok(rc)
            }
        }
    }
    fn walk_pagetree(&self, pos: &mut u32, tree: Ref<PagesNode>,
        func: &mut impl FnMut(u32, &Page), range: &Range<u32>) -> Result<()>
    {
        let node = self.deref(tree)?;
        dbg!(&node);
        match *node {
            PagesNode::Tree(ref tree) => {
                let end = *pos + tree.count as u32; // non-inclusive
                if range.start < end && *pos < range.end {
                    for &k in &tree.kids {
                        self.walk_pagetree(pos, k, func, range)?;
                        if *pos >= range.end {
                            break;
                        }
                    }
                }
                
                *pos = end;
            },
            PagesNode::Leaf(ref page) => {
                if range.contains(pos) {
                    info!("page {}", *pos);
                    func(*pos, page);
                }
                *pos += 1;
            }
        }
        Ok(())
    }
    pub fn pages(&self, mut func: impl FnMut(u32, &Page), range: Range<u32>) -> Result<()> {
        let mut page_nr = 0;
        dbg!(self.get_root()); 
        for &k in &self.get_root().pages.kids {
            dbg!(k);
            self.walk_pagetree(&mut page_nr, k, &mut func, &range)?;
        }
        Ok(())
    }
    pub fn get_num_pages(&self) -> Result<i32> {
        Ok(self.trailer.root.pages.count)
    }
    
    // tail call
    fn find_page(&self, pages: &PageTree, mut offset: i32, page_nr: i32) -> Result<PageRc> {
        for &kid in &pages.kids {
            // println!("{}/{} {:?}", offset, page_nr, kid);
            let rc = self.deref(kid)?;
            match *rc {
                PagesNode::Tree(ref t) => {
                    if offset + t.count < page_nr {
                        offset += t.count;
                    } else {
                        return self.find_page(t, offset, page_nr);
                    }
                },
                PagesNode::Leaf(_) => {
                    if offset < page_nr {
                        offset += 1;
                    } else {
                        assert_eq!(offset, page_nr);
                        return Ok(PageRc(rc));
                    }
                }
            }
        }
        Err(PdfError::PageNotFound {page_nr: page_nr})
    }
    pub fn get_page(&self, n: i32) -> Result<PageRc> {
        if n >= self.get_num_pages()? {
            return Err(PdfError::PageOutOfBounds {page_nr: n, max: self.get_num_pages()?});
        }
        self.find_page(&self.trailer.root.pages, 0, n)
    }

    /*
    pub fn get_images(&self) -> Vec<ImageXObject> {
        let mut images = Vec::<ImageXObject>::new();
        scan_pages(&self.trailer.root.pages, 0, &mut |page| {
            println!("Found page!");
            match page.resources {
                Some(ref res) => {
                    match res.xobject {
                        Some(ref xobjects) => {
                            for (name, xobject) in xobjects {
                                match *xobject {
                                    XObject::Image (ref img_xobject) => {
                                        images.push(img_xobject.clone())
                                    }
                                    _ => {},
                                }
                            }
                        },
                        None => {},
                    }
                },
                None => {},
            }
        });
        images
    }
    
    // tail call to trick borrowck
    fn update_pages(&self, pages: &mut PageTree, mut offset: i32, page_nr: i32, page: Page) -> Result<()>  {
        for kid in &mut pages.kids.iter_mut() {
            // println!("{}/{} {:?}", offset, page_nr, kid);
            match *(self.deref(kid)?) {
                PagesNode::Tree(ref mut t) => {
                    if offset + t.count < page_nr {
                        offset += t.count;
                    } else {
                        return self.update_pages(t, offset, page_nr, page);
                    }
                },
                PagesNode::Leaf(ref mut p) => {
                    if offset < page_nr {
                        offset += 1;
                    } else {
                        assert_eq!(offset, page_nr);
                        *p = page;
                        return Ok(());
                    }
                }
            }
            
        }
        Err(PdfError::PageNotFound {page_nr: page_nr})
    }
    
    pub fn update_page(&mut self, page_nr: i32, page: Page) -> Result<()> {
        self.update_pages(&mut self.trailer.root.pages, 0, page_nr, page)
    }
    
    pub fn update(&mut self, id: ObjNr, primitive: Primitive) {
        self.changes.insert(id, primitive);
    }
    
    pub fn promise<T: Object>(&mut self) -> PromisedRef<T> {
        let id = self.refs.len() as u64;
        
        self.refs.push(XRef::Promised);
        
        PromisedRef {
            inner: PlainRef {
                id:     id,
                gen:    0
            },
            _marker:    PhantomData
        }
    }
    
    pub fn fulfill<T>(&mut self, promise: PromisedRef<T>, obj: T) -> Ref<T>
    where T: Into<Primitive>
    {
        self.update(promise.inner.id, obj.into());
        
        Ref::new(promise.inner)
    }
    
    pub fn add<T>(&mut self, obj: T) -> Ref<T> where T: Into<Primitive> {
        let id = self.refs.len() as u64;
        self.refs.push(XRef::Promised);
        self.update(id, obj.into());
        
        Ref::from_id(id)
    }
    */
}


#[derive(Object, Default)]
pub struct Trailer {
    #[pdf(key = "Size")]
    pub highest_id:         i32,

    #[pdf(key = "Prev")]
    pub prev_trailer_pos:   Option<i32>,

    #[pdf(key = "Root")]
    pub root:               Catalog,

    #[pdf(key = "Encrypt")]
    pub encrypt_dict:       Option<Dictionary>,

    #[pdf(key = "Info")]
    pub info_dict:          Option<Dictionary>,

    #[pdf(key = "ID")]
    pub id:                 Vec<PdfString>,
}

#[derive(Object, Debug)]
#[pdf(Type = "XRef")]
pub struct XRefInfo {
    // XRefStream fields
    #[pdf(key = "Size")]
    pub size: i32,

    //
    #[pdf(key = "Index", default = "vec![0, size]")]
    /// Array of pairs of integers for each subsection, (first object number, number of entries).
    /// Default value (assumed when None): `(0, self.size)`.
    pub index: Vec<i32>,

    #[pdf(key = "Prev")]
    prev: Option<i32>,

    #[pdf(key = "W")]
    pub w: Vec<i32>
}

/*
pub struct XRefStream {
    pub data: Vec<u8>,
    pub info: XRefInfo,
}

impl Object for XRefStream {
    fn serialize<W: io::Write>(&self, _out: &mut W) -> io::Result<()> {
        unimplemented!();
    }
    fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
        let stream = p.to_stream(resolve)?;
        let info = XRefInfo::from_primitive(Primitive::Dictionary (stream.info), resolve)?;
        let data = stream.data.clone();
        Ok(XRefStream {
            data: data,
            info: info,
        })
    }
}
*/
