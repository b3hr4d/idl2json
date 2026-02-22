use clap::Parser;
use idl2json_cli as lib;
use std::io::{self, Read};

/// Reads JSON from stdin, writes candid on stdout.
fn main() {
    let args = lib::Json2IdlArgs::parse();
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .expect("Failed to read string from stdin");
    let idl_str = lib::main_json2idl(&args, &buffer).expect("Failed to convert JSON to IDL");
    println!("{idl_str}");
}
