#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
use colored::Colorize;
use fluent::{FluentBundle, FluentResource};
use quote::quote;
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
use syn::{
    Attribute, File, Item, ItemFn, ItemMacro, ItemStruct, Meta, MetaList, Path as SynPath,
    PathSegment, Type,
};
use unic_langid::langid;

#[derive(Debug, Clone)]
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

    fn get_value_from_slug(&self, slug: &str) -> Option<String> {
        let _format = |v: &str| -> String {
            if !v.contains("\"") {
                format!("\"{}\"", v)
            } else {
                format!("\"{}\"", v.replace("\"", "\\\""))
            }
        };
        if slug == self.slug {
            return Some(_format(&self.value));
        } else {
            // remove the first part split with "_"
            let parts = slug.split("_").skip(1).collect::<Vec<_>>();
            let new_slug = parts.join("_");
            eprintln!("new_slug: {:?}", new_slug);
            for (k, v) in self.childs.iter() {
                if k == slug
                    || k == &format!(".{}", slug)
                    || k == &new_slug
                    || k == &format!(".{}", new_slug)
                {
                    return Some(_format(&v));
                }
            }
        }

        return None;
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
            //eprintln!("now add child: {} => {}", self.cur_key, self.cur_val);
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
        eprintln!("now add entry: {:#?}", entry);
        if self.entries.iter().any(|e| e.slug == entry.slug) {
            panic!("error duplicated: {:#?}", entry);
        }
        self.entries.push(entry);
        self.parent_key.clear();
        self.parent_val.clear();
        self.cur_key.clear();
        self.cur_val.clear();
        self.childs.clear();
    }

    fn parse_lines(&mut self, lines: Vec<String>) {
        for (index, line) in lines.iter().enumerate() {
            let strip = line.trim();
            //eprintln!("now strip: {}", strip);
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
                if strip.ends_with("=") {
                    let key = strip.trim_end_matches("=").trim();
                    if self.parent_key.is_empty() {
                        self.parent_key = key.to_string();
                    } else {
                        self.cur_key = key.to_string();
                    }
                } else {
                    if self.parent_val.is_empty() {
                        self.parent_val = append_to_string(&self.cur_val, &strip);
                    } else {
                        self.cur_val = append_to_string(&self.cur_val, &strip);
                    }
                }
            } else {
                // new entry
                eprintln!("prev line: {:?}", lines[index - 1]);
                eprintln!("current line is empty: {}", line);
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

#[derive(Debug)]
struct ErrorStruct {
    pub slug: Option<String>,
    pub attrs: HashMap<String, String>,
    pub field_labels: Vec<(String, String)>,
    pub sub_diags: Vec<String>,
    pub diag_type: String,
    pub diag_name: String,
    pub parent_diag: Option<String>,
    pub source: String,
}

impl ErrorStruct {
    fn print(&self) {
        eprintln!("--------------------------------");
        eprintln!("slug: {:#?}", self.slug);
        eprintln!("sub_diags: {:#?}", self.sub_diags);
        eprintln!("diag_type: {:#?}", self.diag_type);
        eprintln!("diag_name: {:#?}", self.diag_name);
        eprintln!("parent_diag: {:#?}", self.parent_diag);
        eprintln!("attrs: {:#?}", self.attrs);
        eprintln!("field_labels: {:#?}", self.field_labels);
        eprintln!("source: {:}", self.source);
        eprintln!("--------------------------------");
    }
}

struct SynVisitor {
    pub errors: Vec<ErrorStruct>,
    pub fluent_source: HashMap<String, Entry>,
    pub file_source_code: String,
}

impl SynVisitor {
    fn find_error_by_diag_name(&self, diag_name: &str) -> Option<usize> {
        for (index, error) in self.errors.iter().enumerate() {
            if error.diag_name == diag_name {
                return Some(index);
            }
        }
        None
    }

    fn set_parent_diag(&mut self) {
        let mut map = HashMap::new();
        for error in self.errors.iter() {
            for sub_diag in error.sub_diags.iter() {
                if let Some(index) = self.find_error_by_diag_name(sub_diag) {
                    map.insert(index, error.diag_name.to_string());
                } else {
                    //unreachable!("not found sub_diag: {}", sub_diag);
                }
            }
        }
        for (index, parent) in map.iter() {
            self.errors[*index].parent_diag = Some(parent.to_string());
        }
    }

    fn set_source_code(&mut self) {
        let code = self.file_source_code.to_string();
        let lines = code.lines().collect::<Vec<_>>();
        let mut map = HashMap::new();
        for (index, error) in self.errors.iter().enumerate() {
            let diag_name = error.diag_name.clone();
            let mut start = 0;
            let mut source = vec![];
            for (index, line) in lines.iter().enumerate() {
                if line.contains(&diag_name) && line.starts_with("pub") {
                    start = index;
                    break;
                }
            }
            while start > 0 {
                start -= 1;
                if lines[start].starts_with("#[derive") {
                    break;
                }
            }
            //eprintln!("name: {} start: {}", diag_name, start);
            for (index, line) in lines.iter().enumerate() {
                if index >= start {
                    source.push(line.to_string());
                }
                //eprintln!("now index: {}, start: {}", lines[index], start - 1);
                if index > start && line.trim() == "" && lines[index - 1].trim() == "}" {
                    break;
                }
            }
            map.insert(index, source.join("\n"));
        }
        for (index, source) in map.iter() {
            self.errors[*index].source = source.to_string();
        }
    }

    fn set_fluent_source(&mut self, entries: &Vec<Entry>) {
        for entry in entries.iter() {
            eprintln!("inesrt entry slug {:#?} =>  {:#?}", entry.slug, entry);
            self.fluent_source
                .insert(entry.slug.to_string(), entry.clone());
        }
    }

    fn get_entry_from_slug(&self, error_struct: &ErrorStruct) -> Option<&Entry> {
        let slug = error_struct.slug.clone();
        if let Some(slug) = slug {
            if let Some(entry) = self.fluent_source.get(&slug) {
                eprintln!(
                    "get_entry_from_slug got entry: {:#?}\n by slug: {:?}",
                    entry, slug
                );
                return Some(entry);
            }
        }

        if let Some(parent_name) = &error_struct.parent_diag {
            //eprintln!("got parent_diag: {:?}", parent_name);
            let parent_index = self.find_error_by_diag_name(parent_name).unwrap();
            self.get_entry_from_slug(self.errors.get(parent_index).unwrap())
        } else {
            return None;
        }
    }

    fn gen_source_code(&self) {
        //let mut output = "".to_string();
        let mut error_struct_outputs = vec![];
        for error in self.errors.iter() {
            error.print();
            let entry = self.get_entry_from_slug(error);
            if entry.is_none() {
                eprintln!(
                    "no entry for error: {:#?} slug: {:?}",
                    error.diag_name, error.slug
                );
                continue;
            }
            //eprintln!("entry: {:#?}", entry);
            let entry = entry.unwrap();
            let mut result = error.source.clone();
            let slug = error.slug.clone();
            if let Some(slug) = slug {
                let value = entry.get_value_from_slug(&slug);
                //eprintln!("got slug_value: {:#?}  slug: {:#?}", value, slug);
                if let Some(slug_value) = value {
                    result = result.replace(
                        format!("diag({})", slug).as_str(),
                        format!("diag(text = {})", slug_value).as_str(),
                    );
                    result = replace_slug(&result, &slug, slug_value.as_str());
                }
            }
            let mut add_labels: Vec<(String, String)> = error
                .attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            add_labels.extend(error.field_labels.clone());

            for (name, value) in add_labels.iter() {
                let find_slug = if value == "_" {
                    format!(".{}", name)
                } else {
                    value.to_string()
                };
                let slug_value = entry.get_value_from_slug(&find_slug);
                //eprintln!("got slug_value: {:#?}  slug: {:#?}", slug_value, find_slug);
                if let Some(slug_value) = slug_value {
                    // replace slug with value
                    result = replace_slug(&result, &find_slug, slug_value.as_str());

                    // replace attr with value, like `#[suggestion( ...)]`
                    if value == "_" {
                        result = replace_attr_name(&result, name, slug_value.as_str());
                    }
                }
            }
            //output = format!("{}\n{}", output, result);
            error_struct_outputs.push((error.source.to_string(), result));

            // eprintln!(
            //     "result:-------------------------------------------------------------------\n{}",
            //     result
            // );
        }
        // write the result to file
        let mut output = self.file_source_code.to_string();
        for (from, to) in error_struct_outputs.iter() {
            output = output.replace(from, to);
        }
        eprintln!(
            "result:-------------------------------------------------------------------\n{}",
            output
        );
        fs::write("./result.rs", output).expect("Unable to write to file");
    }
}

impl<'ast> Visit<'ast> for SynVisitor {
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        //eprintln!("Enum with name={}", i.ident);
        let mut slug = None;
        let attrs = HashMap::new();
        let mut field_labels = vec![];
        let mut diag_type = None;
        //let mut sub_diags = vec![];
        let diag_name = i.ident.to_string();

        //eprintln!("attr len: {}", i.attrs.len());
        if let Some(first_attr) = i.attrs.first() {
            diag_type = get_diag_type(first_attr);
            //eprintln!("diag_type: {:?}", diag_type);
        }
        for attr in i.attrs.iter() {
            if attr.path().is_ident("diag")
                || attr.path().is_ident("multipart_suggestion")
                || attr.path().is_ident("suggestion")
            {
                let _ = attr.parse_nested_meta(|meta| {
                    //eprintln!("Attr with name={:#?}", meta.path);
                    let first_segment = meta.path.segments.first().unwrap();
                    let _slug = first_segment.ident.to_string();
                    //eprintln!("Attr with name={:#?}", _slug);
                    if slug.is_none() {
                        slug = Some(_slug);
                    }
                    Ok(())
                });
            }
        }
        for field in i.variants.iter() {
            //eprintln!("field: {:#?}", field);
            for attr in field.attrs.iter() {
                if attr.path().is_ident("subdiagnostic") {
                    let field_name = field.ident.to_string();
                    panic!("subdiagnostic: {}", field_name);
                    //sub_diags.push(field_name);
                }
                let variants = vec![
                    "suggestion",
                    "label",
                    "note",
                    "help",
                    "multipart_suggestion",
                ];
                for key in variants.iter() {
                    eprintln!(
                        "enum attr path: {:?} key: {}, result: {}",
                        attr.path(),
                        key,
                        attr.path().is_ident(key)
                    );
                    if attr.path().is_ident(key) {
                        let mut added = false;
                        eprintln!("try to add field label: {}", key);
                        let _ = attr.parse_nested_meta(|meta| {
                            if let Some(slug_segment) = meta.path.segments.first() {
                                let _slug = slug_segment.ident.to_string();
                                if _slug != "style" && _slug != "code" && _slug != "applicability" {
                                    //eprintln!("add here {} => {:#?}", key, _slug);
                                    field_labels.push((key.to_string(), _slug.to_string()));
                                    added = true;
                                }
                            } else {
                                //eprintln!("Attr with name={:#?}", meta.path);
                                panic!("not found slug");
                            }
                            Ok(())
                        });
                        if !added {
                            //eprintln!("add default label: {}", key);
                            field_labels.push((key.to_string(), "_".to_string()));
                        }
                    }
                }
            }
        }
        if let Some(diag_type) = diag_type {
            let error_struct = ErrorStruct {
                slug,
                attrs,
                sub_diags: vec![],
                field_labels: field_labels,
                diag_type,
                diag_name,
                parent_diag: None,
                source: "".to_string(),
            };
            //eprintln!("error_struct: {:#?}", error_struct);
            self.errors.push(error_struct);
        }
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        //eprintln!("Struct with name={:#?}", i);
        let token_stream = quote! { #i };
        let mut slug = None;
        let mut sub_diags = vec![];
        let mut attrs = HashMap::new();
        let mut field_labels = vec![];
        let mut diag_type = None;
        let diag_name = i.ident.to_string();
        //eprintln!("attr len: {}", i.attrs.len());
        if let Some(first_attr) = i.attrs.first() {
            diag_type = get_diag_type(first_attr);
            //eprintln!("diag_type: {:?}", diag_type);
        }
        for attr in i.attrs.iter() {
            if attr.path().is_ident("diag")
                || attr.path().is_ident("multipart_suggestion")
                || attr.path().is_ident("suggestion")
            {
                let _ = attr.parse_nested_meta(|meta| {
                    //eprintln!("Attr with name={:#?}", meta.path);
                    let first_segment = meta.path.segments.first().unwrap();
                    let _slug = first_segment.ident.to_string();
                    //eprintln!("Attr with name={:#?}", _slug);
                    if slug.is_none() {
                        slug = Some(_slug);
                    }
                    Ok(())
                });
            }
            let keys = vec!["warn", "label", "note", "help"];
            for k in keys.iter() {
                if attr.path().is_ident(k) {
                    let _ = attr.parse_nested_meta(|meta| {
                        //eprintln!("Attr with name={:#?}", meta.path);
                        let first_segment = meta.path.segments.first().unwrap();
                        let _slug = first_segment.ident.to_string();
                        //eprintln!("Attr with name={:#?}", _slug);
                        if slug.is_none() {
                            if !attrs.contains_key(&k.to_string()) {
                                attrs.insert(k.to_string(), _slug);
                            }
                        }
                        Ok(())
                    });
                    if !attrs.contains_key(&k.to_string()) {
                        attrs.insert(k.to_string(), "_".to_string());
                    }
                }
            }
        }
        for field in i.fields.iter() {
            //eprintln!("field: {:#?}", field);
            for attr in field.attrs.iter() {
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
                        let mut added = false;
                        let _ = attr.parse_nested_meta(|meta| {
                            //eprintln!("Attr with name={:#?}", meta.path);
                            if let Some(slug_segment) = meta.path.segments.first() {
                                let _slug = slug_segment.ident.to_string();
                                if _slug != "style" && _slug != "code" && _slug != "applicability" {
                                    eprintln!("add field label {} => {:#?}", key, _slug);
                                    field_labels.push((key.to_string(), _slug));
                                    added = true;
                                }
                            } else {
                                eprintln!("Attr with name={:#?}", meta.path);
                                panic!("not found slug");
                            }
                            Ok(())
                        });
                        if !added {
                            eprintln!("add default label: {}", key);
                            field_labels.push((key.to_string(), "_".to_string()));
                        }
                    }
                }
            }
        }

        if let Some(diag_type) = diag_type {
            let error_struct = ErrorStruct {
                slug,
                attrs,
                sub_diags: sub_diags,
                field_labels,
                diag_type,
                diag_name,
                parent_diag: None,
                source: "".to_string(),
            };
            //eprintln!("error_struct: {:#?}", error_struct);
            self.errors.push(error_struct);
        }
        //eprintln!("struct slug: {:#?}", i);
        visit::visit_item_struct(self, i);
    }
}

fn get_path_first(path: &SynPath) -> String {
    let first_segment = path.segments.first().unwrap();
    return first_segment.ident.to_string();
}

fn get_diag_type(attr: &Attribute) -> Option<String> {
    match attr {
        Attribute {
            meta: Meta::List(MetaList { tokens, .. }),
            ..
        } => {
            let tokens = tokens.to_string();
            Some(tokens)
        }
        _ => None,
    }
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
                        //eprintln!("subdiagnostic path: {:?}", path);
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

fn replace_slug(content: &str, slug: &str, to: &str) -> String {
    let mut result = content.to_string();
    let from = format!("({})", slug);
    if result.contains(from.as_str()) {
        result = result.replace(from.as_str(), format!("({})", to).as_str());
    } else {
        result = result.replace(
            format!("{}", slug).as_str(),
            format!("label = {} ", to).as_str(),
        );
    };
    return result;
}

fn replace_attr_name(content: &str, attr: &str, to: &str) -> String {
    let mut result = content.to_string();
    let from = format!("#[{}]", attr);
    if result.contains(from.as_str()) {
        result = result.replace(from.as_str(), format!("#[{}({})]", attr, to).as_str());
    } else {
        result = result.replace(
            format!("{}(", attr).as_str(),
            format!("{}(label = {}, ", attr, to).as_str(),
        );
    };
    return result;
}

fn append_to_string(prev: &str, add: &str) -> String {
    if prev.is_empty() {
        return add.to_string();
    }
    format!("{}\n{}", prev, add)
}

fn try_main() -> Result<(), Error> {
    let mut args = env::args_os();
    let _ = args.next(); // executable name

    let path = std::env::args().nth(1).expect("No file provided");
    let content = std::fs::read_to_string(&path).expect("read failed");

    let lines = content.lines().map(|s| s.to_string()).collect::<Vec<_>>();
    let parser = &mut Parser::new();
    parser.parse_lines(lines.to_vec());

    let code_file_path = std::env::args().nth(2).expect("No file provided");
    let code = fs::read_to_string(&code_file_path).map_err(Error::ReadFile)?;
    let syntax = syn::parse_file(&code).map_err({
        |error| Error::ParseFile {
            error,
            filepath: code_file_path.clone().into(),
            source_code: code.clone(),
        }
    })?;
    //println!("{:#?}", syntax);
    let visitor = &mut SynVisitor {
        errors: vec![],
        fluent_source: HashMap::new(),
        file_source_code: code.to_string(),
    };
    visitor.visit_file(&syntax);
    visitor.set_parent_diag();
    visitor.set_source_code();
    // for error in visitor.errors.iter() {
    //     eprintln!("error: {:#?}", error);
    //     eprintln!("{}", error.source);
    // }
    // for entry in parser.entries.iter() {
    //     eprintln!("haha entry: {:#?}\n\n", entry);
    // }
    visitor.set_fluent_source(&parser.entries);
    visitor.gen_source_code();
    Ok(())
}

fn main() {
    if let Err(error) = try_main() {
        let _ = writeln!(io::stderr(), "{}", error);
        process::exit(1);
    }
}
