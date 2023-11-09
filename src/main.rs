#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
use colored::Colorize;
use fluent::{FluentBundle, FluentResource};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use syn::token::Struct;
use syn::visit::{self, Visit};
use syn::{File, Item, ItemFn, ItemMacro, ItemStruct, Path as SynPath, PathSegment, Type};
use unic_langid::langid;

struct Entry {
    slug: String,
    value: String,
    childs: Vec<(String, String)>,
}

impl Entry {
    fn new(slug: String, value: String) -> Self {
        Self {
            slug,
            value,
            childs: Vec::new(),
        }
    }

    fn add_child(&mut self, slug: String, value: String) {
        self.childs.push((slug, value));
    }

    fn print(&self) {
        println!("{} = {}", self.slug, self.value);
        for (slug, value) in self.childs.iter() {
            println!("-- {} = {}", slug, value);
        }
        println!("--------------\n\n");
    }
}

#[derive(Default)]
struct Parser {
    entries: Vec<Entry>,
    childs: Vec<(String, String)>,
    cur_key: String,
    cur_val: String,
    parent_key: String,
    parent_val: String,
}

impl Parser {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            ..Default::default()
        }
    }

    fn print(&self) {
        for entry in self.entries.iter() {
            entry.print();
        }
    }

    fn add_child(&mut self) {
        if !self.cur_key.is_empty() {
            self.childs
                .push((self.cur_key.to_string(), self.cur_val.to_string()));
        }
    }

    fn add_entry(&mut self) {
        if self.parent_key.is_empty() {
            return;
        }
        self.add_child();
        let mut entry = Entry::new(self.parent_key.to_string(), self.parent_val.to_string());
        for (k, v) in self.childs.iter() {
            entry.add_child(k.to_string(), v.to_string());
        }
        self.entries.push(entry);
        self.parent_key.clear();
        self.parent_val.clear();
        self.cur_key.clear();
        self.cur_val.clear();
        self.childs.clear();
    }

    fn parse_lines(&mut self, lines: Vec<String>) {
        for line in lines {
            let strip = line.trim();
            if let Some((k, v)) = check_kv(strip) {
                if k.starts_with(".") {
                    // child entry
                    self.add_child();
                    self.cur_key = k.to_string();
                    self.cur_val = v.to_string();
                } else {
                    // new entry
                    self.add_entry();
                    self.parent_key = k.to_string();
                    self.parent_val = v.to_string();
                }
            } else if !strip.is_empty() {
                // continue line
                let add = format!("\n{}", line).to_string();
                if !self.cur_val.is_empty() {
                    self.cur_val.push_str(&add);
                } else {
                    self.parent_val.push_str(&add);
                }
            } else {
                // new entry
                self.add_entry();
            }
        }
        self.add_entry();
    }
}

fn check_kv(input: &str) -> Option<(&str, &str)> {
    let re = Regex::new(r"(?m)^([\._a-zA-Z][_a-zA-Z0-9]*?) = (.*?)$").unwrap();
    if re.is_match(input) {
        let caps = re.captures(input).unwrap();
        let slug = caps.get(1).unwrap().as_str();
        let value = caps.get(2).unwrap().as_str();
        Some((slug, value))
    } else {
        None
    }
}

fn run_conver() {
    // read the input fluent file from arguments
    let path = std::env::args().nth(1).expect("No file provided");
    let content = std::fs::read_to_string(&path).expect("read failed");

    let lines = content.lines().map(|s| s.to_string()).collect::<Vec<_>>();
    let parser = &mut Parser::new();
    parser.parse_lines(lines.to_vec());
    parser.print();
}

fn test_now() {
    // let ftl_string = "exp_unmatched_angle = unmatched angle {$plural ->
    //     [true] brackets
    //     *[false] bracket
    // }"
    // .to_owned();
    let ftl_string = "exp_unmatched_angle = remove extra angle {$plural ->
        [true] brackets
        *[false] bracket
        }"
    .to_owned();

    let res = FluentResource::try_new(ftl_string).expect("Failed to parse an FTL string.");

    let langid_en = langid!("en-US");
    let mut bundle = FluentBundle::new(vec![langid_en]);

    bundle
        .add_resource(&res)
        .expect("Failed to add FTL resources to the bundle.");

    let msg = bundle
        .get_message("exp_unmatched_angle")
        .expect("Message doesn't exist.");
    let mut errors = vec![];
    let pattern = msg.value().expect("Message has no value.");
    let mut args = fluent::FluentArgs::new();
    args.set("plural", "false");
    args.set("yukang", "yukang text");
    let value = bundle.format_pattern(&pattern, Some(&args), &mut errors);
    eprintln!("value: {}", value);
}

enum Error {
    IncorrectUsage,
    ReadFile(io::Error),
    ParseFile {
        error: syn::Error,
        filepath: PathBuf,
        source_code: String,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            IncorrectUsage => write!(f, "Usage: dump-syntax path/to/filename.rs"),
            ReadFile(error) => write!(f, "Unable to read file: {}", error),
            ParseFile {
                error,
                filepath,
                source_code,
            } => write!(f, "error: {} {:#?} {}", error, filepath, source_code),
        }
    }
}

fn main() {
    if let Err(error) = try_main() {
        let _ = writeln!(io::stderr(), "{}", error);
        process::exit(1);
    }
}

struct SynVisitor;

impl<'ast> Visit<'ast> for SynVisitor {
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        eprintln!("Enum with name={}", i.ident);
        let mut slug = None;
        let mut attrs = HashMap::new();
        for attr in i.attrs.iter() {
            if attr.path().is_ident("diag") || attr.path().is_ident("multipart_suggestion") {
                let _ = attr.parse_nested_meta(|meta| {
                    //eprintln!("Attr with name={:#?}", meta.path);
                    let first_segment = meta.path.segments.first().unwrap();
                    let _slug = first_segment.ident.to_string();
                    //eprintln!("Attr with name={:#?}", _slug);
                    slug = Some(_slug);
                    Ok(())
                });
            }
        }
        for field in i.variants.iter() {
            //eprintln!("field: {:#?}", field);
            if let Some(attr) = field.attrs.first() {
                if attr.path().is_ident("subdiagnostic") {
                    let field_name = field.ident.to_string();
                    panic!("subdiagnostic: {}", field_name);
                }
                let variants = vec!["suggestion", "label", "note", "help"];
                for key in variants.iter() {
                    if attr.path().is_ident(key) {
                        let _ = attr.parse_nested_meta(|meta| {
                            if let Some(slug_segment) = meta.path.segments.first() {
                                let _slug = slug_segment.ident.to_string();
                                if !attrs.contains_key(&key.to_string())
                                    && _slug != "style"
                                    && _slug != "code"
                                    && _slug != "applicability"
                                {
                                    eprintln!(" {} => {:#?}", key, _slug);
                                    attrs.insert(key.to_string(), _slug);
                                }
                                if !attrs.contains_key(&key.to_string()) {
                                    attrs.insert(key.to_string(), "_".to_string());
                                }
                            } else {
                                eprintln!("Attr with name={:#?}", meta.path);
                                panic!("not found slug");
                            }
                            Ok(())
                        });
                    }
                }
            }
        }
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        eprintln!("Struct with name={}", i.ident);
        let mut slug = None;
        let mut sub_diags = vec![];
        let mut attrs = HashMap::new();
        //eprintln!("attr len: {}", i.attrs.len());
        for attr in i.attrs.iter() {
            if attr.path().is_ident("diag") || attr.path().is_ident("multipart_suggestion") {
                let _ = attr.parse_nested_meta(|meta| {
                    //eprintln!("Attr with name={:#?}", meta.path);
                    let first_segment = meta.path.segments.first().unwrap();
                    let _slug = first_segment.ident.to_string();
                    //eprintln!("Attr with name={:#?}", _slug);
                    slug = Some(_slug);
                    Ok(())
                });
            }
        }
        for field in i.fields.iter() {
            //eprintln!("field: {:#?}", field);
            if let Some(attr) = field.attrs.last() {
                if attr.path().is_ident("subdiagnostic") {
                    let field_name = field.ident.as_ref().unwrap().to_string();
                    let field_ty = &field.ty;
                    //eprintln!("subdiagnostic: {} {:#?}", field_name, field_ty);
                    let subdiag_struct = get_ty_path(field_ty);
                    sub_diags.push(subdiag_struct);
                }
                let variants = vec!["suggestion", "label", "note", "help"];
                for key in variants.iter() {
                    //eprintln!("now attr.path() : {:#?}", attr.path());
                    if attr.path().is_ident(key) {
                        let _ = attr.parse_nested_meta(|meta| {
                            //eprintln!("Attr with name={:#?}", meta.path);
                            if let Some(slug_segment) = meta.path.segments.first() {
                                let _slug = slug_segment.ident.to_string();
                                if !attrs.contains_key(&key.to_string())
                                    && _slug != "style"
                                    && _slug != "code"
                                    && _slug != "applicability"
                                {
                                    eprintln!(" {} => {:#?}", key, _slug);
                                    attrs.insert(key.to_string(), _slug);
                                }
                                if !attrs.contains_key(&key.to_string()) {
                                    eprintln!("insert empty now {}", key);
                                    attrs.insert(key.to_string(), "_".to_string());
                                }
                            } else {
                                eprintln!("Attr with name={:#?}", meta.path);
                                panic!("not found slug");
                            }
                            Ok(())
                        });
                    }
                }
            }
        }
        //eprintln!("struct slug: {:#?}", i);
        visit::visit_item_struct(self, i);
    }
}

fn get_path_first(path: &SynPath) -> String {
    let first_segment = path.segments.first().unwrap();
    return first_segment.ident.to_string();
}

fn get_ty_path(ty: &Type) -> String {
    match ty {
        Type::Path(path) => {
            let first = path.path.segments.first().unwrap();
            if first.ident == "Option" {
                let segments = &path.path.segments;
                let segment = segments.first().unwrap();
                if let PathSegment {
                    ident,
                    arguments:
                        syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                            args,
                            ..
                        }),
                } = segment
                {
                    if let syn::GenericArgument::Type(Type::Path(path)) = args.first().unwrap() {
                        let path = get_path_first(&path.path);
                        eprintln!("subdiagnostic path: {:?}", path);
                        return path;
                    }
                }
            } else {
                return first.ident.to_string();
            }
        }
        _ => (),
    }
    return "".to_string();
}

fn try_main() -> Result<(), Error> {
    let mut args = env::args_os();
    let _ = args.next(); // executable name

    let filepath = match (args.next(), args.next()) {
        (Some(arg), None) => PathBuf::from(arg),
        _ => return Err(Error::IncorrectUsage),
    };

    let code = fs::read_to_string(&filepath).map_err(Error::ReadFile)?;
    let syntax = syn::parse_file(&code).map_err({
        |error| Error::ParseFile {
            error,
            filepath,
            source_code: code,
        }
    })?;
    //println!("{:#?}", syntax);
    SynVisitor.visit_file(&syntax);
    Ok(())
}
