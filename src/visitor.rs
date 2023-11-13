use crate::utils::get_diag_type;
use crate::utils::{replace_attr_name, replace_slug};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, ItemStruct};
#[derive(Debug)]
pub struct ErrorStruct {
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
        eprintln!("--------------------------------");
    }
}

pub struct SynVisitor {
    pub errors: Vec<ErrorStruct>,
    pub fluent_source: HashMap<String, crate::Entry>,
    pub file_source_code: String,
    pub cur_item_name: Vec<(String, String)>,
    pub cur_source: Vec<String>,
    pub attrs: HashMap<String, Vec<Attribute>>,
    pub path_replace: Vec<String>,
}

impl SynVisitor {
    pub fn init_with_syntax(&mut self, syntax: &syn::File) {
        self.visit_file(&syntax);
        self.set_parent_diag();
    }

    pub fn find_error_by_diag_name(&self, diag_name: &str) -> Option<usize> {
        for (index, error) in self.errors.iter().enumerate() {
            if error.diag_name == diag_name {
                return Some(index);
            }
        }
        None
    }

    pub fn set_parent_diag(&mut self) {
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

    pub fn set_fluent_source(&mut self, entries: &Vec<crate::Entry>) {
        let mut entries = entries.clone();
        let kv: HashMap<String, String> = entries
            .iter()
            .map(|e| (e.slug.to_string(), e.value.to_string()))
            .collect();

        let childs = entries
            .iter()
            .map(|e| (e.slug.to_string(), e.value.to_string()))
            .collect::<Vec<_>>();
        let root_entry = crate::Entry {
            slug: "*root*".to_string(),
            value: "".to_string(),
            childs: childs.clone(),
        };
        entries.push(root_entry.clone());

        let fix_vars = |value: &str| {
            let re = Regex::new(r"\{(\w+)\}").unwrap();
            let mut change = vec![];
            for mat in re.captures_iter(value) {
                if let Some(v) = kv.get(&mat[1]) {
                    eprintln!("now change {:#?} => {:#?}", &mat[1], v);
                    change.push((mat[1].to_string(), v.to_string()));
                }
            }
            let mut fixed = value.to_string();
            for (mat, value) in change.iter() {
                let from = format!("{{{}}}", mat);
                fixed = fixed.replace(&from, value);
            }
            fixed
        };
        for i in 0..entries.len() {
            let entry = entries.get_mut(i).unwrap();
            let mut new_childs = vec![];
            for (slug, value) in entry.childs.iter() {
                let fixed = fix_vars(value);
                new_childs.push((slug.to_string(), fixed));
            }
            entry.childs = new_childs;
            entry.value = fix_vars(&entry.value);
        }

        for entry in entries.iter() {
            //eprintln!("insert entry slug {:#?} =>  {:#?}", entry.slug, entry);
            self.fluent_source
                .insert(entry.slug.to_string(), entry.clone());
        }

        self.fluent_source.get("parse_invalid_char_in_escape");
    }

    fn get_entry_from_struct(&self, error_struct: &ErrorStruct) -> Option<&crate::Entry> {
        let slug = error_struct.slug.clone();
        if let Some(slug) = slug {
            if let Some(entry) = self.fluent_source.get(&slug) {
                return Some(entry);
            }
        }

        if let Some(parent_name) = &error_struct.parent_diag {
            let parent_index = self.find_error_by_diag_name(parent_name).unwrap();
            self.get_entry_from_struct(self.errors.get(parent_index).unwrap())
        } else {
            return self.fluent_source.get("*root*");
        }
    }

    fn get_value(&self, error: &ErrorStruct, slug: &str) -> Option<String> {
        if let Some(entry) = self.get_entry_from_struct(error) {
            if let Some(v) = entry.get_value_from_slug(slug) {
                return Some(v);
            }
        } else {
            if let Some(parent_name) = &error.parent_diag {
                let parent_index = self.find_error_by_diag_name(parent_name).unwrap();
                if let Some(parent) =
                    self.get_entry_from_struct(self.errors.get(parent_index).unwrap())
                {
                    if let Some(v) = parent.get_value_from_slug(slug) {
                        return Some(v);
                    }
                }
            }
        }
        let root = self.fluent_source.get("*root*").unwrap();
        root.get_value_from_slug(slug)
    }

    pub fn gen_source_code(&self) -> String {
        //let mut output = "".to_string();
        let mut error_struct_outputs = vec![];
        for error in self.errors.iter() {
            //error.print();
            let Some(entry) = self.get_entry_from_struct(error) else {
                eprintln!(
                    "no entry for error: {:#?} slug: {:?}",
                    error.diag_name, error.slug
                );
                continue;
            };
            let mut result = error.source.clone();
            let slug = error.slug.clone();

            let mut add_labels: Vec<(String, String)> = error
                .attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            add_labels.extend(error.field_labels.clone());

            if let Some(slug) = slug {
                let value = self.get_value(&error, &slug);
                if let Some(slug_value) = value {
                    add_labels.push((slug, slug_value));
                }
            }

            add_labels.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

            for (name, value) in add_labels.iter() {
                let find_slug = if value == "_" {
                    format!(".{}", name)
                } else {
                    value.to_string()
                };
                let slug_value = self.get_value(&error, &find_slug);
                eprintln!(
                    "all_labels find_slug: {:#?} => slug_value: {:#?}",
                    find_slug, slug_value
                );
                if let Some(slug_value) = slug_value {
                    if value == "_" {
                        result = replace_attr_name(&result, name, slug_value.as_str());
                    } else {
                        result = replace_slug(&result, &find_slug, slug_value.as_str());
                    }
                } else {
                    eprintln!("not found slug_value: {}", find_slug);
                    eprintln!(
                        "not found slug_value find_slug: {:#?} => slug_value: {:#?}",
                        find_slug, slug_value
                    );
                    //panic!("none error now");
                }
            }
            error_struct_outputs.push((error.source.to_string(), result));
        }
        // write the result to file
        let mut output = self.file_source_code.to_string();
        for (from, to) in error_struct_outputs.iter() {
            output = output.replace(from, to);
        }

        let root = self.fluent_source.get("*root*").unwrap();
        let mut cur_entry = root.clone();
        for path in self.path_replace.iter() {
            let elems = path.split("::").collect::<Vec<_>>();
            if elems.len() == 2 && elems[0] == "fluent" {
                eprintln!("path: {:#?}", elems);
                let slug = elems[1];
                if let Some(entry) = self.fluent_source.get(slug) {
                    cur_entry = entry.clone();
                }
                let value = cur_entry.get_value_from_slug(slug).unwrap();
                //let value = root.get_value_from_slug(slug).unwrap();
                let replace = format!("DiagnosticMessage::Str(Cow::from({}))", &value);
                output = output.replace(path, &replace);
            }
        }
        return output;
    }

    fn cur_diag_name(&self) -> Option<String> {
        let last = self.cur_item_name.last();
        if let Some((name, ty)) = last {
            if ty == "Enum" || ty == "Struct" {
                return Some(name.to_string());
            } else {
                // a variant, try to find previous item
                if self.cur_item_name.len() >= 2 {
                    for (parent_name, ty) in self.cur_item_name.iter().rev() {
                        if ty == "Enum" || ty == "Struct" {
                            let new_name = format!("{}::{}", parent_name, name);
                            return Some(new_name);
                        }
                    }
                }
            }
        }
        return None;
    }

    fn process_attrs(&mut self, sub_diags: &Vec<String>) {
        let mut slug = None;
        let diag_attrs = HashMap::new();
        let mut field_labels = BTreeSet::new();
        let mut diag_type = None;

        let Some(diag_name) = self.cur_diag_name() else {
            return;
        };

        let Some(attrs) = self.attrs.get(&diag_name) else {
            return;
        };
        if let Some(first_attr) = attrs.first() {
            diag_type = get_diag_type(first_attr);
        }
        for attr in attrs.iter() {
            if attr.path().is_ident("diag")
                || attr.path().is_ident("multipart_suggestion")
                || attr.path().is_ident("suggestion")
            {
                let _ = attr.parse_nested_meta(|meta| {
                    let first_segment = meta.path.segments.first().unwrap();
                    let _slug = first_segment.ident.to_string();
                    if slug.is_none() {
                        slug = Some(_slug);
                    }
                    Ok(())
                });
            }
        }
        for attr in attrs.iter() {
            let variants = vec![
                "suggestion",
                "label",
                "note",
                "help",
                "multipart_suggestion",
                "diag",
            ];
            for key in variants.iter() {
                if attr.path().is_ident(key) {
                    let mut added = false;
                    let _ = attr.parse_nested_meta(|meta| {
                        if let Some(slug_segment) = meta.path.segments.first() {
                            let _slug = slug_segment.ident.to_string();
                            if _slug != "style" && _slug != "code" && _slug != "applicability" {
                                field_labels.insert((key.to_string(), _slug.to_string()));
                                added = true;
                            }
                        } else {
                            panic!("not found slug");
                        }
                        Ok(())
                    });
                    if !added {
                        field_labels.insert((key.to_string(), "_".to_string()));
                    }
                }
            }
        }
        let parent_diag = if self.cur_item_name.len() >= 2 {
            self.cur_item_name
                .get(self.cur_item_name.len() - 2)
                .map(|x| x.0.to_string())
        } else {
            None
        };
        if let Some(diag_type) = diag_type {
            let error_struct = ErrorStruct {
                slug,
                attrs: diag_attrs,
                sub_diags: sub_diags.clone(),
                field_labels: field_labels.into_iter().collect(),
                diag_type,
                diag_name,
                parent_diag,
                source: self.cur_source.last().unwrap().to_string(),
            };
            //eprintln!("error_struct: {:#?}", error_struct);
            self.errors.push(error_struct);
        }
    }
}

impl<'ast> Visit<'ast> for SynVisitor {
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        if let Some(diag_name) = self.cur_diag_name() {
            self.attrs
                .entry(diag_name)
                .or_insert_with(|| Vec::new())
                .push(i.clone());
        }
        visit::visit_attribute(self, i);
    }

    fn visit_path(&mut self, i: &'ast syn::Path) {
        eprintln!("visiting path: {:#?}", i);
        visit::visit_path(self, i);
        let source = i.span().source_text().unwrap().to_string();
        eprintln!("path source: {}", source);
        self.path_replace.push(source);
    }

    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        eprintln!("Enum with name={:#?}", i.ident.to_string());
        let span = i.span();
        let source = i.span().source_text().unwrap().to_string();
        self.cur_item_name
            .push((i.ident.to_string(), "Enum".to_string()));
        self.cur_source.push(source);

        self::visit::visit_item_enum(self, i);
        self.process_attrs(&vec![]);
        self.cur_item_name.pop();
        self.cur_source.pop();
    }

    fn visit_variant(&mut self, i: &'ast syn::Variant) {
        eprintln!("Variant with name={:#?}", i.ident.to_string());
        let source = i.span().source_text().unwrap().to_string();
        self.cur_item_name
            .push((i.ident.to_string(), "Variant".to_string()));
        self.cur_source.push(source);

        let mut sub_diags = vec![];
        for field in i.fields.iter() {
            for attr in field.attrs.iter() {
                if attr.path().is_ident("subdiagnostic") {
                    let field_name = field.ident.as_ref().unwrap().to_string();
                    let field_ty = &field.ty;
                    //eprintln!("subdiagnostic: {} {:#?}", field_name, field_ty);
                    let subdiag_struct = crate::utils::get_ty_path(field_ty);
                    sub_diags.push(subdiag_struct);
                }
            }
        }
        self::visit::visit_variant(self, i);
        self.process_attrs(&sub_diags);
        self.cur_item_name.pop();
        self.cur_source.pop();
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        eprintln!("Struct with name={:#?}", i.ident.to_string());
        self.cur_item_name
            .push((i.ident.to_string(), "Struct".to_string()));
        self.cur_source
            .push(i.span().source_text().unwrap().to_string());
        let mut sub_diags = vec![];
        for field in i.fields.iter() {
            for attr in field.attrs.iter() {
                if attr.path().is_ident("subdiagnostic") {
                    let field_name = field.ident.as_ref().unwrap().to_string();
                    let field_ty = &field.ty;
                    //eprintln!("subdiagnostic: {} {:#?}", field_name, field_ty);
                    let subdiag_struct = crate::utils::get_ty_path(field_ty);
                    sub_diags.push(subdiag_struct);
                }
            }
        }

        self::visit::visit_item_struct(self, i);
        self.process_attrs(&sub_diags);
        self.cur_item_name.pop();
        self.cur_source.pop();
    }
}
