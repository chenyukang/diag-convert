#![allow(unused_variables)]
#![allow(dead_code)]
use crate::parser::Parser;
use crate::visitor::SynVisitor;
use std::collections::HashMap;
use std::fs;
use std::io::Error;

pub fn gen_code(ftl_file: &str, errors_path: &str, output: Option<String>) -> Result<(), Error> {
    let content = std::fs::read_to_string(&ftl_file).expect("read failed");
    let lines = content.lines().map(|s| s.to_string()).collect::<Vec<_>>();
    let parser = &mut Parser::new();
    parser.parse_lines(lines.to_vec());

    let code = fs::read_to_string(&errors_path)?;
    let syntax = syn::parse_file(&code).unwrap();
    let visitor = &mut SynVisitor {
        errors: vec![],
        fluent_source: HashMap::new(),
        file_source_code: code.to_string(),
        attrs: HashMap::new(),
        cur_item_name: vec![],
    };
    visitor.init_with_syntax(&syntax);

    visitor.set_fluent_source(&parser.entries);
    let result = visitor.gen_source_code();
    if let Some(output) = output {
        if result.contains("_in_raw_string") {
            eprintln!("fuck !!");
        }
        fs::write(output, result)?;
    } else {
        println!("{}", result);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_gen_code() {
        let ftl_file = "tests/case1/test.ftl";
        let errors_path = "tests/case1/test.rs";
        let output = "/tmp/output.rs";
        let _ = fs::remove_file(output);
        let _ = gen_code(ftl_file, errors_path, Some(output.to_string()));
        let result = fs::read_to_string(output).unwrap();
        let expected = fs::read_to_string("tests/case1/expect.rs").unwrap();
        assert_eq!(result, expected);
    }
}
