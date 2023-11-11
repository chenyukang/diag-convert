use crate::utils::get_diag_type;
use crate::utils::{replace_attr_name, replace_slug};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
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
        eprintln!("source: {:}", self.source);
        eprintln!("--------------------------------");
    }
}

pub struct SynVisitor {
    pub errors: Vec<ErrorStruct>,
    pub fluent_source: HashMap<String, crate::Entry>,
    pub file_source_code: String,
    pub cur_item_name: Vec<String>,
    pub attrs: HashMap<String, Vec<Attribute>>,
}

impl SynVisitor {
    pub fn init_with_syntax(&mut self, syntax: &syn::File) {
        self.visit_file(&syntax);
        self.set_parent_diag();
        self.set_source_code();
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

    pub fn set_source_code(&mut self) {
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

    pub fn set_fluent_source(&mut self, entries: &Vec<crate::Entry>) {
        for entry in entries.iter() {
            eprintln!("insert entry slug {:#?} =>  {:#?}", entry.slug, entry);
            self.fluent_source
                .insert(entry.slug.to_string(), entry.clone());
        }
        let childs = entries
            .iter()
            .map(|e| (e.slug.to_string(), e.value.to_string()))
            .collect::<Vec<_>>();
        let mut fixed_childs = vec![];
        for (slug, value) in childs.iter() {
            let re = Regex::new(r"\{(\w+)\}").unwrap();
            let mut change = vec![];
            for mat in re.captures_iter(value) {
                for (k, v) in childs.iter() {
                    if k == &mat[1] {
                        change.push((mat[1].to_string(), v.to_string()));
                        break;
                    }
                }
            }
            let mut fixed = value.to_string();
            for (mat, value) in change.iter() {
                let from = format!("{{{}}}", mat);
                fixed = fixed.replace(&from, value);
            }
            fixed_childs.push((slug.to_string(), fixed));
        }
        let root_entry = crate::Entry {
            slug: "*root*".to_string(),
            value: "".to_string(),
            childs: fixed_childs,
        };
        self.fluent_source.insert("*root*".to_string(), root_entry);
        // set root variables
    }

    fn get_entry_from_slug(&self, error_struct: &ErrorStruct) -> Option<&crate::Entry> {
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
            return self.fluent_source.get("*root*");
        }
    }

    fn get_slug_value(&self, entry: &crate::Entry, slug: &str) -> Option<String> {
        if let Some(value) = entry.get_value_from_slug(slug) {
            return Some(value);
        }
        let root = self.fluent_source.get("*root*").unwrap();
        return root.get_value_from_slug(slug);
    }

    pub fn gen_source_code(&self) -> String {
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
                let value = self.get_slug_value(&entry, &slug);
                //eprintln!("got slug_value: {:#?}  slug: {:#?}", value, slug);
                if let Some(slug_value) = value {
                    result = result.replace(
                        format!("diag({})", slug).as_str(),
                        format!("diag(label = {})", slug_value).as_str(),
                    );
                    result = replace_slug(&result, "", &slug, slug_value.as_str());
                }
            }
            let mut add_labels: Vec<(String, String)> = error
                .attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            add_labels.extend(error.field_labels.clone());

            add_labels.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

            for (name, value) in add_labels.iter() {
                let find_slug = if value == "_" {
                    format!(".{}", name)
                } else {
                    value.to_string()
                };
                let slug_value = self.get_slug_value(&entry, &find_slug);
                if let Some(slug_value) = slug_value {
                    // replace slug with value
                    eprintln!(
                        "try to replace name {:#?}  find_slug:{:#?}  value: {:#?}",
                        name, find_slug, slug_value
                    );
                    result = replace_slug(&result, &name, &find_slug, slug_value.as_str());

                    // replace attr with value, like `#[suggestion( ...)]`
                    if value == "_" {
                        result = replace_attr_name(&result, name, slug_value.as_str());
                    }
                }
            }
            error_struct_outputs.push((error.source.to_string(), result));
        }
        // write the result to file
        let mut output = self.file_source_code.to_string();
        for (from, to) in error_struct_outputs.iter() {
            output = output.replace(from, to);
        }
        return output;
    }

    fn cur_diag_name(&self) -> Option<String> {
        self.cur_item_name.last().map(|x| x.to_string())
    }

    fn process_attrs(&mut self, sub_diags: &Vec<String>) {
        let mut slug = None;
        let diag_attrs = HashMap::new();
        let mut field_labels = BTreeSet::new();
        let mut diag_type = None;
        let diag_name = &self.cur_item_name;

        let Some(diag_name) = self.cur_diag_name() else {
            return;
        };

        let Some(attrs) = self.attrs.get(&diag_name) else {
            return;
        };
        eprintln!("diag_name now: {:?}", diag_name);
        //eprintln!("attr len: {}", i.attrs.len());
        if let Some(first_attr) = attrs.first() {
            diag_type = get_diag_type(first_attr);
        }
        for attr in attrs.iter() {
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
        //eprintln!("field: {:#?}", field);
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
                    eprintln!("try to add field label: {}", key);
                    let _ = attr.parse_nested_meta(|meta| {
                        if let Some(slug_segment) = meta.path.segments.first() {
                            let _slug = slug_segment.ident.to_string();
                            if _slug != "style" && _slug != "code" && _slug != "applicability" {
                                //eprintln!("add here {} => {:#?}", key, _slug);
                                field_labels.insert((key.to_string(), _slug.to_string()));
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
                        field_labels.insert((key.to_string(), "_".to_string()));
                    }
                }
            }
        }
        if let Some(diag_type) = diag_type {
            let error_struct = ErrorStruct {
                slug,
                attrs: diag_attrs,
                sub_diags: sub_diags.clone(),
                field_labels: field_labels.into_iter().collect(),
                diag_type,
                diag_name: self.cur_item_name.last().unwrap().to_string(),
                parent_diag: None,
                source: "".to_string(),
            };
            //eprintln!("error_struct: {:#?}", error_struct);
            self.errors.push(error_struct);
        }
    }
}

impl<'ast> Visit<'ast> for SynVisitor {
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        eprintln!("visiting attr: {:#?}", i);
        if let Some(diag_name) = self.cur_diag_name() {
            self.attrs
                .entry(diag_name)
                .or_insert_with(|| Vec::new())
                .push(i.clone());
        }
        visit::visit_attribute(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        eprintln!("Enum with name={:#?}", i.ident.to_string());
        self.cur_item_name.push(i.ident.to_string());
        self::visit::visit_item_enum(self, i);
        self.process_attrs(&vec![]);
        self.cur_item_name.pop();
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        eprintln!("Struct with name={:#?}", i.ident.to_string());
        self.cur_item_name.push(i.ident.to_string());
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
        visit::visit_item_struct(self, i);
    }
}
