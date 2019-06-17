//! `pdf_derive` provides a proc macro to derive the Object trait from the `pdf` crate.
//! # Usage
//! There are several ways to derive Object on a struct or enum:
//! ## 1. Struct from PDF Dictionary
//!
//! A lot of dictionary types defined in the PDF 1.7 reference have a finite amount of possible
//! fields. Each of these are usually either required or optional. The latter is achieved by using
//! a `Option<T>` or `Vec<T>` as type of a field.
//!
//! Usually, dictionary types
//! require that the entry `/Type` is some specific string. By default, `pdf_derive` assumes that
//! this should equal the name of the input struct. This can be overridden by setting the `Type`
//! attribute equal to either the expected value of the `/Type` entry, or to `false` in order to
//! omit the type check completly.
//!
//! Check similar to that of `/Type` can also be specified in the same manner. (but the `Type`
//! attribute is special because it accepts a bool).
//!
//! Examples:
//!
//! ```norun
//! #[derive(Object)]
//! #[pdf(Type="XObject", Subtype="Image")]
//! /// A variant of XObject
//! pub struct ImageDictionary {
//!     #[pdf(key="Width")]
//!     width: i32,
//!     #[pdf(key="Height")]
//!     height: i32,
//!     // [...]
//! }
//! ```
//!
//! This enforces that the dictionary's `/Type` entry is present and equals `/XObject`, and that the
//! `/Subtype` entry is present and equals `/Image`.
//!
//! Each field in the struct needs to implement `Object`. Implementation is provided already for
//! common types like i32, f32, usize, bool, String (from Primitive::Name), Option<T> and Vec<T>.
//! The two latter are initialized to default if the entry isn't found in the input dictionary.
//! Option<T> is therefore frequently used for fields that are optional according to the PDF
//! reference. Vec<T> can also be used for optional fields that can also be arrays (there are quite
//! a few of those in the PDF specs - one or many). However, as stated, it accepts absense of the
//! entry, so **required** fields of type array aren't yet facilitated for.
//!
//! Lastly, for each field, it's possible to define a default value by setting the `default`
//! attribute to a string that can parse as Rust code.
//!
//! Example:
//!
//! ```norun
//! #[derive(Object)]
//! #[pdf(Type = "XRef")]
//! pub struct XRefInfo {
//!     #[pdf(key = "Filter")]
//!     filter: Vec<StreamFilter>,
//!     #[pdf(key = "Size")]
//!     pub size: i32,
//!     #[pdf(key = "Index", default = "vec![0, size]")]
//!     pub index: Vec<i32>,
//!     // [...]
//! }
//! ```
//!
//!
//! ## 2. Struct from PDF Stream
//! PDF Streams consist of a stream dictionary along with the stream itself. It is assumed that all
//! structs that want to derive Object where the primitive it  converts from is a stream,
//! have a field `info: T`, where `T: Object`, and a field `data: Vec<u8>`.
//!
//! Deriving an Object that converts from Primitive::Stream, the flag `is_stream` is required in
//! the proc macro attributes.
//!
//! ## 3. Enum from PDF Name
//! Example:
//!
//! ```norun
//! #[derive(Object, Debug)]
//! pub enum StreamFilter {
//!     ASCIIHexDecode,
//!     ASCII85Decode,
//!     LZWDecode,
//!     FlateDecode,
//!     JPXDecode,
//!     DCTDecode,
//! }
//! ```
//!
//! In this case, `StreamFilter::from_primitive(primitive)` will return Ok(_) only if the primitive
//! is `Primitive::Name` and matches one of the enum variants
#![recursion_limit="128"]

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use syn::*;
type SynStream = syn::export::TokenStream2;

// Debugging:
/*
use std::fs::{OpenOptions};
use std::io::Write;
*/






#[proc_macro_derive(Object, attributes(pdf))]
pub fn object(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    // Build the impl
    impl_object(&ast)
}

struct FieldAttrs {
    key: LitStr,
    default: Option<Expr>
}

/// Returns None if the field is to be skipped
fn field_attrs(field: &Field) -> Option<FieldAttrs> {
    field.attrs.iter()
    .filter_map(|attr| {
        if attr.path.is_ident("pdf") {
            let list = match attr.parse_meta() {
                Ok(Meta::List(list)) => list,
                Ok(_) => panic!("only #[pdf(attrs...)] is allowed"),
                Err(e) => panic!("can't parse meta attributes: {}", e)
            };
            let (mut key, mut default, mut skip) = (None, None, false);
            for meta in list.nested.iter() {
                match *meta {
                    NestedMeta::Meta(Meta::NameValue(MetaNameValue { ref ident, lit: Lit::Str(ref value), ..}))
                    if ident == "key"
                        => key = Some(value.clone()),
                    NestedMeta::Meta(Meta::NameValue(MetaNameValue { ref ident, lit: Lit::Str(ref value), ..}))
                    if ident == "default"
                        => default = Some(value.clone()),
                    NestedMeta::Meta(Meta::Word(ref ident))
                    if ident == "skip"
                        => skip = true,
                    _ => panic!(r##"Derive error - Supported derive attributes: `key="Key"`, `default="some code"`."##)
                }
            }
            Some(match skip {
                true => None,
                false => Some(FieldAttrs {
                    key: key.expect("attr `key` missing"),
                    default: default.map(|s| parse_str(&s.value()).expect("can't parse `default` as EXPR"))
                }),
            })
        } else {
            None 
        }
    }).next().expect("no pdf meta attribute")
}




/// Just the attributes for the whole struct
#[derive(Default)]
struct GlobalAttrs {
    /// List of checks to do in the dictionary (LHS is the key, RHS is the expected value)
    checks: Vec<(String, String)>,
    type_name: Option<String>,
    type_required: bool,
    is_stream: bool
}
impl GlobalAttrs {
    /// The PDF type may be explicitly specified as an attribute with type "Type". Else, it is the name
    /// of the struct.
    fn from_ast(ast: &DeriveInput) -> GlobalAttrs {
        let mut attrs = GlobalAttrs::default();
        
        for attr in ast.attrs.iter().filter(|attr| attr.path.is_ident("pdf")) {
            let list = match attr.parse_meta() {
                Ok(Meta::List(list)) => list,
                Ok(_) => panic!("only #[pdf(attrs...)] is allowed"),
                Err(e) => panic!("can't parse meta attributes: {}", e)
            };
            
            // Loop through list of attributes
            for meta in list.nested.iter() {
                match *meta {
                    NestedMeta::Meta(Meta::NameValue(MetaNameValue { ref ident, ref lit, ..})) => {
                        if ident == "Type" {
                            match lit {
                                Lit::Str(ref value) => {
                                    let mut value = value.value();
                                    attrs.type_required = if value.ends_with("?") {
                                        value.pop(); // remove '?'
                                        true
                                    } else {
                                        false
                                    };
                                    attrs.type_name = Some(value);
                                },
                                _ => panic!("Value of 'Type' attribute must be a String."),
                            }
                        } else {
                            match lit {
                                Lit::Str(ref value) => attrs.checks.push((ident.to_string(), value.value())),
                                _ => panic!("Other checks must have RHS String."),
                            }
                        }
                    },
                    NestedMeta::Meta(Meta::Word(ref ident)) if ident == "is_stream" => attrs.is_stream = true,
                    _ => {}
                }
            }
        }

        attrs
    }
}

fn impl_object(ast: &DeriveInput) -> TokenStream {
    let attrs = GlobalAttrs::from_ast(&ast);
    match (attrs.is_stream, &ast.data) {
        (true, Data::Struct(ref data)) => impl_object_for_stream(ast, &data.fields).into(),
        (false, Data::Struct(ref data)) => impl_object_for_struct(ast, &data.fields).into(),
        (true, Data::Enum(ref variants)) => impl_enum_from_stream(ast, variants, &attrs).into(),
        (false, Data::Enum(ref variants)) => impl_object_for_enum(ast, variants).into(),
        (_, _) => unimplemented!()
    }
}
/// Accepts Name to construct enum
fn impl_object_for_enum(ast: &DeriveInput, data: &DataEnum) -> SynStream {
    let id = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let ser_code: Vec<_> = data.variants.iter().map(|var| {
        quote! {
            #id::#var => stringify!(#id::#var),
        }
    }).collect();

    let from_primitive_code = impl_from_name(id, data);
    quote! {
        impl #impl_generics crate::object::Object for #id #ty_generics #where_clause {
            fn serialize<W: ::std::io::Write>(&self, out: &mut W) -> ::std::io::Result<()> {
                writeln!(out, "/{}",
                    match *self {
                        #( #ser_code )*
                    }
                )
            }
            fn from_primitive(p: Primitive, _resolve: &dyn Resolve) -> Result<Self> {
                #from_primitive_code
            }
        }
    }
}

/// Returns code for from_primitive that accepts Name
fn impl_from_name(id: &Ident, data: &DataEnum) -> SynStream {
    let parts: Vec<_> = data.variants.iter().map(|var| {
        quote! {
            stringify!(#var) => Ok(#id::#var),
        }
    }).collect();
    quote! {
        match p {
            Primitive::Name(name) => {
                match name.as_str() {
                    #( #parts )*
                    s => Err(crate::PdfError::UnknownVariant { id: stringify!(#id), name: s.to_string() }),
                }
            }
            _ => Err(crate::PdfError::UnexpectedPrimitive { expected: "Name", found: p.get_debug_name() }),
        }
    }
}

fn impl_enum_from_stream(ast: &DeriveInput, data: &DataEnum, attrs: &GlobalAttrs) -> SynStream {
    let id = &ast.ident;
    
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let ty_check = match (&attrs.type_name, attrs.type_required) {
        (Some(ref ty), required) => quote! {
            stream.info.expect("Type", #ty, #required)?;
        },
        (None, _) => quote!{}
    };
    
    let variants_code: Vec<_> = data.variants.iter().map(|var| {
        let inner_ty = match var.fields {
            Fields::Unnamed(ref fields) => {
                assert_eq!(fields.unnamed.len(), 1, "all variants in a stream enum have to have exactly one unnamed field");
                fields.unnamed.first().unwrap().value().ty.clone()
            },
            _ => panic!("all variants in a stream enum have to have exactly one unnamed field")
        };
        quote! {
            stringify!(#var) => Ok(#id::#var ( #inner_ty::from_primitive(Primitive::Stream(stream), resolve)?)),
        }
    }).collect();
    
    quote! {
        impl #impl_generics crate::object::Object for #id #ty_generics #where_clause {
            fn serialize<W: ::std::io::Write>(&self, out: &mut W) -> ::std::io::Result<()> {
                unimplemented!();
            }
            fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
                let mut stream = PdfStream::from_primitive(p, resolve)?;
                #ty_check
                
                let subty = stream.info.get("Subtype")
                    .ok_or(PdfError::MissingEntry { typ: stringify!(#id), field: "Subtype".into()})?
                    .as_name()?;
                
                match subty.as_str() {
                    #( #variants_code ),*
                    s => Err(crate::PdfError::UnknownVariant { id: stringify!(#id), variant: s.into() })
                }
            }
        }
    }
}

/// Accepts Dictionary to construct a struct
fn impl_object_for_struct(ast: &DeriveInput, fields: &Fields) -> SynStream {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let attrs = GlobalAttrs::from_ast(&ast);

    let parts: Vec<_> = fields.iter()
    .map(|field| {
        (field.ident.clone(), field_attrs(field))
    }).collect();
    
    // Implement serialize()
    let fields_ser = parts.iter()
    .map( |&(ref field, ref attrs)|
        if let Some(attrs) = attrs.as_ref() {
            let ref key = attrs.key;
            quote! {
                write!(out, "{} ", #key)?;
                self.#field.serialize(out)?;
                writeln!(out, "")?;
            }
        } else {
            quote!()
        }
    );
    let checks_code = attrs.checks.iter().map(|&(ref key, ref val)|
        quote! {
            writeln!(out, "/{} /{}", #key, #val)?;
        }
    );

    ///////////////////////
    let typ = name.to_string();
    let let_parts = fields.iter().map(|field| {
        let FieldAttrs { default, key } = match field_attrs(field) {
            Some(attrs) => attrs,
            None => return quote! {}
        };
        let ref name = field.ident;

        let ty = field.ty.clone();

        if let Some(ref default) = default {
            quote! {
                let #name = {
                    let primitive: Option<crate::primitive::Primitive>
                        = dict.remove(#key);
                    let x: #ty = match primitive {
                        Some(primitive) => <#ty as Object>::from_primitive(primitive, resolve).map_err(|e| 
                            crate::PdfError::FromPrimitive {
                                typ: #typ,
                                field: stringify!(#name),
                                source: Box::new(e)
                            })?,
                        None => #default,
                    };
                    x
                };
            }
        } else {
            quote! {
                let #name = {
                    match dict.remove(#key) {
                        Some(primitive) =>
                            match <#ty as Object>::from_primitive(primitive, resolve) {
                                Ok(obj) => obj,
                                Err(e) => return Err(crate::PdfError::FromPrimitive {
                                    typ: stringify!(#ty),
                                    field: stringify!(#name),
                                    source: Box::new(e)
                                })
                            }
                        None =>  // Try to construct T from Primitive::Null
                            match <#ty as Object>::from_primitive(crate::primitive::Primitive::Null, resolve) {
                                Ok(obj) => obj,
                                Err(_) => return Err(crate::PdfError::MissingEntry {
                                    typ: stringify!(#ty),
                                    field: String::from(stringify!(#name)),
                                })
                            },
                    }
                    // ^ By using Primitive::Null when we don't find the key, we allow 'optional'
                    // types like Option and Vec to be constructed from non-existing values
                };
            }
        }
    });
    
    let field_parts = fields.iter().map(|field| {
        let ref name = field.ident;
        quote! { #name: #name, }
    });
    
    let checks: Vec<_> = attrs.checks.iter().map(|&(ref key, ref val)|
        quote! {
            dict.expect(#key, #val, true)?;
        }
    ).collect();

    let ty_check = match (&attrs.type_name, attrs.type_required) {
        (Some(ref ty), required) => quote! {
            dict.expect("Type", #ty, #required)?;
        },
        (None, _) => quote!{}
    };
        
    
    let from_primitive_code = quote! {
        let mut dict = Dictionary::from_primitive(p, resolve)?;
        #ty_check
        #( #checks )*
        #( #let_parts )*
        Ok(#name {
            #( #field_parts )*
        })
    };
    
    let pdf_type = match attrs.type_name {
        Some(ref ty) => quote! { writeln!(out, "/Type /{}", #ty)?; },
        None => quote! {}
    };
    
    quote! {
        impl #impl_generics crate::object::Object for #name #ty_generics #where_clause {
            fn serialize<W: ::std::io::Write>(&self, out: &mut W) -> ::std::io::Result<()> {
                writeln!(out, "<<")?;
                #pdf_type
                #( #checks_code )*
                #(#fields_ser)*
                writeln!(out, ">>")?;
                Ok(())
            }
            fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
                #from_primitive_code
            }
        }
    }
}

/// Note: must have info and dict (TODO explain in docs)
fn impl_object_for_stream(ast: &DeriveInput, fields: &Fields) -> SynStream {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let info_ty = fields.iter()
    .filter_map(|field| {
        if let Some(ident) = field.ident.as_ref() {
            if ident == "info" {
                Some(field.ty.clone())
            } else {
                None
            }
        } else {
            None
        }
    }).next().unwrap();

    quote! {
        impl #impl_generics crate::object::Object for #name #ty_generics #where_clause {
            fn serialize<W: ::std::io::Write>(&self, _out: &mut W) -> ::std::io::Result<()> {
                unimplemented!();
                /*
                writeln!(out, "<<")?;
                #type_code
                #(#fields_ser)*
                writeln!(out, ">>")?;
                Ok(())
                */
            }
            fn from_primitive(p: Primitive, resolve: &dyn Resolve) -> Result<Self> {
                let crate::primitive::PdfStream {info, data}
                    = p.to_stream(resolve)?;

                Ok(#name {
                    info: <#info_ty as Object>::from_primitive(crate::primitive::Primitive::Dictionary (info), resolve)?,
                    data: data,
                })
            }
        }
    }
}
