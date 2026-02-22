//! Command line library for converting candid to JSON.
#![warn(missing_docs)]
#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::unimplemented)]

#[cfg(test)]
mod tests;

use anyhow::{anyhow, Context};
use candid::types::value::IDLValue;
use candid_parser::{
    parse_idl_args,
    types::{IDLProg, IDLType, IDLTypes},
    IDLArgs,
};
use clap::Parser;
use idl2json::{
    idl2json, idl2json_with_weak_names, idl_args2json_with_weak_names, json2idl_with_type,
    json2idl_with_type_name, json_args2idl_with_types, polyfill, BytesFormat, Idl2JsonOptions,
};
use std::{path::PathBuf, str::FromStr};

fn load_did_files(dids: &[PathBuf]) -> anyhow::Result<Vec<IDLProg>> {
    dids.iter()
        .map(|did| {
            let did_as_str = std::fs::read_to_string(did)
                .with_context(|| anyhow!("Could not read did file '{}'.", did.display()))?;
            IDLProg::from_str(&did_as_str)
                .with_context(|| anyhow!("Failed to parse did file '{}'", did.display()))
        })
        .collect()
}

/// Reads IDL from stdin, writes JSON to stdout.
pub fn main(args: &Args, idl_str: &str) -> anyhow::Result<String> {
    let idl_args: IDLArgs = parse_idl_args(idl_str).with_context(|| anyhow!("Malformed input"))?;
    let idl2json_options = Idl2JsonOptions {
        prog: load_did_files(&args.did)?,
        bytes_as: args.bytes_as,
        compact: args.compact,
        ..Idl2JsonOptions::default()
    };
    // Decide what to do
    if args.init {
        // Use the type of the .did file init arg.
        // - If multiple did files are provided, the first is used.
        // - Clap should reject commands without a --did file.
        let idl_types = polyfill::idl_prog::get_init_arg_type(
            idl2json_options
                .prog
                .first()
                .context("Please specify which .did file to use.")?,
        )
        .context("Failed to get the service argument from the did file.")?;
        serde_json::to_string(&idl_args2json_with_weak_names(
            &idl_args,
            &idl_types,
            &idl2json_options,
        ))
        .context("Failed to serialize to json")
    } else if let Some(idl_type) = &args.typ {
        if idl_type.trim().starts_with('(') {
            let idl_types = IDLTypes::from_str(idl_type).context("Failed to parse type")?;
            serde_json::to_string(&idl_args2json_with_weak_names(
                &idl_args,
                &idl_types,
                &idl2json_options,
            ))
            .context("Failed to serialize to json")
        } else {
            let idl_type = IDLType::from_str(idl_type).context("Failed to parse type")?;
            convert_all(&idl_args, &Some(idl_type), &idl2json_options)
        }
    } else {
        convert_all(&idl_args, &None, &idl2json_options)
    }
}

/// Reads JSON from stdin, writes candid to stdout.
pub fn main_json2idl(args: &Json2IdlArgs, json_str: &str) -> anyhow::Result<String> {
    let did_prog = load_did_files(&args.did)?
        .into_iter()
        .next()
        .unwrap_or(IDLProg {
            decs: vec![],
            actor: None,
        });

    if args.init {
        let init_arg_types = polyfill::idl_prog::get_init_arg_type(&did_prog)
            .context("Failed to get the service argument from the did file.")?;
        json_args2idl_with_types(did_prog, &init_arg_types, json_str)
    } else if let Some(typ) = &args.typ {
        if typ.trim().starts_with('(') {
            let idl_types = IDLTypes::from_str(typ).context("Failed to parse type")?;
            json_args2idl_with_types(did_prog, &idl_types, json_str)
        } else {
            let trimmed_typ = typ.trim();
            if !trimmed_typ.contains(char::is_whitespace)
                && !trimmed_typ.contains('{')
                && !trimmed_typ.contains('}')
                && !trimmed_typ.contains(':')
                && !trimmed_typ.contains(';')
            {
                json2idl_with_type_name(did_prog, trimmed_typ, json_str)
            } else {
                let idl_type = IDLType::from_str(trimmed_typ).context("Failed to parse type")?;
                json2idl_with_type(did_prog, &idl_type, json_str)
            }
        }
    } else {
        Err(anyhow!(
            "Please specify --typ or --init when converting JSON to IDL."
        ))
    }
}

/// Candid typically comes as a tuple of values.  This converts a single value in such a tuple.
fn convert_one(
    idl_value: &IDLValue,
    idl_type: &Option<IDLType>,
    idl2json_options: &Idl2JsonOptions,
) -> anyhow::Result<String> {
    let json_value = if let Some(idl_type) = idl_type {
        idl2json_with_weak_names(idl_value, idl_type, idl2json_options)
    } else {
        idl2json(idl_value, idl2json_options)
    };
    (if idl2json_options.compact {
        serde_json::to_string
    } else {
        serde_json::to_string_pretty
    })(&json_value)
    .with_context(|| anyhow!("Cannot print to stderr"))
}

/// Candid typically comes as a tuple of values.  This converts all such tuples
fn convert_all(
    idl_args: &IDLArgs,
    idl_type: &Option<IDLType>,
    idl2json_options: &Idl2JsonOptions,
) -> anyhow::Result<String> {
    let json_structures: anyhow::Result<Vec<String>> = idl_args
        .args
        .iter()
        .map(|idl_value| convert_one(idl_value, idl_type, idl2json_options))
        .collect();
    Ok(json_structures?.join("\n"))
}

/// Converts Candid on stdin to JSON on stdout.
#[derive(Parser, Debug, Default)]
#[clap(name("idl2json"), version = concat!(env!("CARGO_PKG_VERSION"), "\ncandid ", env!("CARGO_CANDID_VERSION")))]
pub struct Args {
    /// A .did file containing type definitions
    #[clap(short, long)]
    did: Vec<PathBuf>,
    /// The name of a type in the provided .did file
    #[clap(short, long)]
    typ: Option<String>,
    /// Use the service init argument type from the did file
    #[clap(short, long, requires("did"))]
    init: bool,
    /// How to display bytes
    #[clap(short, long, value_enum)]
    bytes_as: Option<BytesFormat>,
    /// Print compact output
    #[clap(short, long)]
    compact: bool,
}

/// Converts JSON on stdin to Candid on stdout.
#[derive(Parser, Debug, Default)]
#[clap(name("json2idl"), version = concat!(env!("CARGO_PKG_VERSION"), "\ncandid ", env!("CARGO_CANDID_VERSION")))]
pub struct Json2IdlArgs {
    /// A .did file containing type definitions
    #[clap(short, long)]
    did: Vec<PathBuf>,
    /// The name of a type in the provided .did file or a candid type literal
    #[clap(short, long)]
    typ: Option<String>,
    /// Use the service init argument type from the did file
    #[clap(short, long, requires("did"))]
    init: bool,
}
