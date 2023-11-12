#![allow(unused_variables)]
#![allow(dead_code)]
use crate::parser::Parser;
use crate::visitor::SynVisitor;
use std::collections::HashMap;
use std::fs;
use std::io::Error;
use std::process::Command;

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
        let output_path = "/tmp/output.rs";
        let expected_path = "tests/case1/expect.rs";
        let _ = fs::remove_file(output_path);
        let _ = gen_code(ftl_file, errors_path, Some(output_path.to_string()));
        let result = fs::read_to_string(output_path).unwrap();
        let expected = fs::read_to_string(expected_path).unwrap();
        if result != expected {
            // run diff to show the differences of the two files
            let res = Command::new("diff")
                .arg(expected_path)
                .arg(output_path)
                .output()
                .expect("failed to execute diff");

            println!("Diff output:\n{}", String::from_utf8_lossy(&res.stdout));
            panic!("the result is diff from expected result ...");
        }
    }
}
