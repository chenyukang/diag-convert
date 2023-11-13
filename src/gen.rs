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
        cur_source: vec![],
        path_replace: vec![],
    };
    visitor.init_with_syntax(&syntax);

    visitor.set_fluent_source(&parser.entries);
    let result = visitor.gen_source_code();
    if let Some(output) = output {
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
    use std::process::Command;

    fn single_test(ftl_file: &str, code_path: &str, expected_path: &str, output_path: &str) {
        let _ = fs::remove_file(output_path);
        let _ = gen_code(ftl_file, code_path, Some(output_path.to_string()));
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
            eprintln!("diff cmd: diff {} {}", expected_path, output_path);
            panic!("the result is diff from expected result ...");
        }
    }

    #[test]
    fn test_gen_code() {
        single_test(
            "tests/case1/test.ftl",
            "tests/case1/test.rs",
            "tests/case1/expect.rs",
            "/tmp/errors-gen.rs",
        );
    }

    #[test]
    fn test_path_gen() {
        single_test(
            "tests/case1/test.ftl",
            "tests/path-fix/input.rs",
            "tests/path-fix/expect.rs",
            "/tmp/path-gen.rs",
        );
    }
}
