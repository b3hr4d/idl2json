use anyhow::{anyhow, Context};
use candid_parser::{
    types::{IDLType, IDLTypes},
    IDLArgs, IDLProg,
};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use yaml2candid::Yaml2Candid;

fn json_str_to_value(json_str: &str) -> anyhow::Result<JsonValue> {
    serde_json::from_str(json_str).with_context(|| anyhow!("Malformed input"))
}

fn json_value_to_yaml_value(json_value: &JsonValue) -> anyhow::Result<YamlValue> {
    serde_yaml::to_value(json_value).context("Failed to convert JSON value")
}

fn convert_one(
    converter: &Yaml2Candid,
    typ: &IDLType,
    json_value: &JsonValue,
) -> anyhow::Result<String> {
    let yaml_value = json_value_to_yaml_value(json_value)?;
    let idl_value = converter.convert(typ, &yaml_value)?;
    Ok(idl_value.to_string())
}

/// Converts one JSON value to one candid value using a named type from a .did file.
pub fn json2idl_with_type_name(
    prog: IDLProg,
    type_name: &str,
    json_str: &str,
) -> anyhow::Result<String> {
    let converter = Yaml2Candid { prog };
    let json_value = json_str_to_value(json_str)?;
    convert_one(
        &converter,
        &IDLType::VarT(type_name.to_string()),
        &json_value,
    )
}

/// Converts one JSON value to one candid value using a literal type.
pub fn json2idl_with_type(
    prog: IDLProg,
    idl_type: &IDLType,
    json_str: &str,
) -> anyhow::Result<String> {
    let converter = Yaml2Candid { prog };
    let json_value = json_str_to_value(json_str)?;
    convert_one(&converter, idl_type, &json_value)
}

/// Converts one JSON array to candid args using a tuple/list of candid types.
pub fn json_args2idl_with_types(
    prog: IDLProg,
    idl_types: &IDLTypes,
    json_str: &str,
) -> anyhow::Result<String> {
    let converter = Yaml2Candid { prog };
    let json_value = json_str_to_value(json_str)?;
    let json_args = json_value
        .as_array()
        .ok_or_else(|| anyhow!("Expected a JSON array"))?;
    if json_args.len() != idl_types.args.len() {
        return Err(anyhow!(
            "Expected {} JSON values for candid args but got {}",
            idl_types.args.len(),
            json_args.len()
        ));
    }
    let args = json_args
        .iter()
        .zip(idl_types.args.iter())
        .map(|(value, typ)| {
            let yaml_value = json_value_to_yaml_value(value)?;
            converter.convert(typ, &yaml_value)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(IDLArgs { args }.to_string())
}
