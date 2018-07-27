// Parses DWARF information.

use std::collections::HashMap;

use gimli;

use gimli::{DebugAbbrev, DebugInfo, DebugLine, DebugStr, LittleEndian};

trait Reader: gimli::Reader<Offset = usize> {}

impl<'input, Endian> Reader for gimli::EndianBuf<'input, Endian>
where
    Endian: gimli::Endianity,
{
}

use wasm_read::DebugSections;

fn to_vec(b: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b);
    result
}

pub struct DebugLoc {
    pub address: u64,
    pub source_id: u32,
    pub line: u32,
    pub column: u32,
}

pub struct DebugLocInfo {
    pub sources: Vec<String>,
    pub locations: Vec<DebugLoc>,
    pub sources_content: Option<Vec<String>>,
}

pub fn get_debug_loc(debug_sections: &DebugSections) -> DebugLocInfo {
    let mut sources = Vec::new();
    let mut locations = Vec::new();
    let mut source_to_id_map: HashMap<u64, usize> = HashMap::new();

    let ref tables = debug_sections.tables;
    let ref debug_str = DebugStr::new(&tables[&to_vec(b".debug_str")], LittleEndian);
    let ref debug_abbrev = DebugAbbrev::new(&tables[&to_vec(b".debug_abbrev")], LittleEndian);
    let ref debug_info = DebugInfo::new(&tables[&to_vec(b".debug_info")], LittleEndian);
    let ref debug_line = DebugLine::new(&tables[&to_vec(b".debug_line")], LittleEndian);

    let mut iter = debug_info.units();
    while let Some(unit) = iter.next().unwrap_or(None) {
        let abbrevs = unit.abbreviations(debug_abbrev).unwrap();
        let mut cursor = unit.entries(&abbrevs);
        cursor.next_dfs().expect("???");
        let root = cursor.current().expect("missing die");
        let offset = match root.attr_value(gimli::DW_AT_stmt_list).unwrap() {
            Some(gimli::AttributeValue::DebugLineRef(offset)) => offset,
            _ => continue,
        };
        let comp_dir = root.attr(gimli::DW_AT_comp_dir)
            .unwrap()
            .and_then(|attr| attr.string_value(debug_str));
        let comp_name = root.attr(gimli::DW_AT_name)
            .unwrap()
            .and_then(|attr| attr.string_value(debug_str));
        let program = debug_line.program(offset, unit.address_size(), comp_dir, comp_name);
        let mut block_start_loc = locations.len();
        if let Ok(program) = program {
            let mut rows = program.rows();
            while let Some((header, row)) = rows.next_row().unwrap() {
                let pc = debug_sections.code_content as u64 + row.address();
                let line = row.line().unwrap_or(0);
                let column = match row.column() {
                    gimli::ColumnType::Column(column) => column,
                    gimli::ColumnType::LeftEdge => 0,
                };
                let file_index = row.file_index();
                let source_id = if !source_to_id_map.contains_key(&file_index) {
                    let file_path: String = if let Some(file) = row.file(header) {
                        if let Some(directory) = file.directory(header) {
                            format!(
                                "{}/{}",
                                directory.to_string_lossy(),
                                file.path_name().to_string_lossy()
                            )
                        } else {
                            String::from(file.path_name().to_string_lossy())
                        }
                    } else {
                        String::from("<unknown>")
                    };
                    let index = sources.len();
                    sources.push(file_path);
                    source_to_id_map.insert(file_index, index);
                    index
                } else {
                    *source_to_id_map.get(&file_index).unwrap() as usize
                };
                let loc = DebugLoc {
                    address: pc,
                    source_id: source_id as u32,
                    line: line as u32,
                    column: column as u32,
                };
                locations.push(loc);
                if row.end_sequence() {
                    // Heuristic to remove dead functions.
                    let block_end_loc = locations.len() - 1;
                    let fn_size = locations[block_end_loc].address - locations[block_start_loc].address + 1;
                    let fn_size_field_len = ((fn_size + 1).next_power_of_two().trailing_zeros() + 6) / 7;
                    if locations[block_start_loc].address <= debug_sections.code_content as u64 + fn_size_field_len as u64 {
                        locations.drain(block_start_loc..);
                    }
                    block_start_loc = locations.len();
                }
            }
        }

        // new unit, new sources
        source_to_id_map.clear();
    }

    locations.sort_by(|a, b| a.address.cmp(&b.address));

    DebugLocInfo {
        sources,
        locations,
        sources_content: None,
    }
}
