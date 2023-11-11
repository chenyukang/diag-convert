#![allow(unused_variables)]
#![allow(dead_code)]
mod entry;
mod parser;
mod utils;
use entry::Entry;
use std::env;
use std::io::Error;
use std::io::{self, Write};
use std::process;
mod gen;
mod visitor;

fn try_main() -> Result<(), Error> {
    let mut args = env::args_os();
    let _ = args.next();

    let path = std::env::args().nth(1).expect("No file provided");
    let code_file_path = std::env::args().nth(2).expect("No file provided");
    let output = std::env::args().nth(3);
    let result = gen::gen_code(&path, &code_file_path, output);
    Ok(())
}

fn main() {
    if let Err(error) = try_main() {
        let _ = writeln!(io::stderr(), "{}", error);
        process::exit(1);
    }
}
