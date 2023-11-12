use regex::Regex;
use syn::{Attribute, Meta, MetaList, Path as SynPath, PathSegment, Type};

pub fn replace_slug(content: &str, name: &str, slug: &str, to: &str) -> String {
    let mut result = content.to_string();
    let from = format!("({})", slug);
    if name == "diag" {
        result = result.replace(
            format!("diag({})", slug).as_str(),
            format!("diag(label = {})", to).as_str(),
        );
    } else if result.contains(from.as_str()) {
        result = result.replace(from.as_str(), format!("({})", to).as_str());
    } else {
        result = result.replace(
            format!("{}", slug).as_str(),
            format!("label = {} ", to).as_str(),
        );
    };

    return result;
}

pub fn replace_attr_name(content: &str, attr: &str, to: &str) -> String {
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

pub fn append_to_string(prev: &str, add: &str) -> String {
    if prev.is_empty() {
        return add.to_string();
    }
    format!("{}\n{}", prev, add)
}

pub(crate) fn check_kv(input: &str) -> Option<(&str, &str)> {
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

pub fn get_path_first(path: &SynPath) -> String {
    let first_segment = path.segments.first().unwrap();
    return first_segment.ident.to_string();
}

pub fn get_diag_type(attr: &Attribute) -> Option<String> {
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

pub fn get_ty_path(ty: &Type) -> String {
    match ty {
        Type::Path(path) => {
            let first = get_path_first(&path.path);
            if first == "Option" {
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
                        return get_path_first(&path.path);
                    }
                }
            } else {
                return first.to_string();
            }
        }
        _ => (),
    }
    return "".to_string();
}
