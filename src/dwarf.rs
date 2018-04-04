// Parses DWARF information.

use gimli;

use gimli::{
    DebugAbbrev, DebugInfo, DebugStr, DebugLine, LittleEndian
};

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

pub fn print_debug_loc(debug_sections: &DebugSections) {
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
        let comp_dir = root.attr(gimli::DW_AT_comp_dir).unwrap()
            .and_then(|attr| attr.string_value(debug_str));
        let comp_name = root.attr(gimli::DW_AT_name).unwrap()
            .and_then(|attr| attr.string_value(debug_str));
        let program = debug_line.program(offset, unit.address_size(), comp_dir, comp_name);
        if let Ok(program) = program {
            let mut rows = program.rows();
            let mut file_index = 0;
            while let Some((header, row)) = rows.next_row().unwrap() {
                let pc = debug_sections.code_content as u64 + row.address();
                let line = row.line().unwrap_or(0);
                let column = match row.column() {
                    gimli::ColumnType::Column(column) => column,
                    gimli::ColumnType::LeftEdge => 0,
                };
                if file_index != row.file_index() {
                    file_index = row.file_index();
                    if let Some(file) = row.file(header) {
                        if let Some(directory) = file.directory(header) {
                            println!(
                                "uri: \"{}/{}\"",
                                directory.to_string_lossy(),
                                file.path_name().to_string_lossy()
                            );
                        } else {
                            println!("uri: \"{}\"", file.path_name().to_string_lossy());
                        }
                    }
                }
                println!("{:x} @ {},{}", pc, line, column);
            }
        }
    }
}