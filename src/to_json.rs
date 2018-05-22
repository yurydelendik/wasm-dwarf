// Converts DebugLocInfo to JS source maps.

use dwarf::DebugLocInfo;
use rustc_serialize::json::{Json, ToJson};
use std::collections::BTreeMap;
use std::str;
use vlq::encode;

pub fn convert_debug_info_to_json(di: &DebugLocInfo) -> Json {
    let mut buffer = Vec::new();
    let mut last_address = 0;
    let mut last_source_id = 0;
    let mut last_line = 1;
    let mut last_column = 1;
    for loc in di.locations.iter() {
        if loc.line == 0 || loc.column == 0 {
            continue;
        }
        let address_delta = loc.address as i64 - last_address;
        encode(address_delta, &mut buffer).unwrap();
        let source_id_delta = loc.source_id as i64 - last_source_id;
        encode(source_id_delta, &mut buffer).unwrap();
        let line_delta = loc.line as i64 - last_line;
        encode(line_delta, &mut buffer).unwrap();
        let column_delta = loc.column as i64 - last_column;
        encode(column_delta, &mut buffer).unwrap();
        buffer.push(b',');

        last_address = loc.address as i64;
        last_source_id = loc.source_id as i64;
        last_line = loc.line as i64;
        last_column = loc.column as i64;
    }

    if di.locations.len() > 0 {
        buffer.pop();
    }

    let mappings = str::from_utf8(&buffer).unwrap();
    let names: Vec<String> = Vec::new();

    let mut root = BTreeMap::new();
    root.insert("version".to_string(), 3.to_json());
    root.insert("sources".to_string(), di.sources.to_json());
    root.insert("names".to_string(), names.to_json());
    root.insert("mappings".to_string(), mappings.to_json());
    if let Some(ref sources_content) = di.sources_content {
        root.insert("sourcesContent".to_string(), sources_content.to_json());
    }
    Json::Object(root)
}
