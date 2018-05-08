// Applies reloction entries to the existing sections.
use std::collections::HashMap;

use wasmparser::BinaryReader;

use wasm_read::DebugSections;

fn to_vec(b: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b);
    result
}

enum SymbolKind {
    Section (u32),
    Data (u32, u32),
}

pub fn reloc(debug_sections: &mut DebugSections) {
    let (func_indices, symbols) = {
        let ref linking_table = debug_sections.linking.as_ref().unwrap();
        let mut reader = BinaryReader::new(&linking_table);
        let version = reader.read_var_u32().unwrap();
        assert!(version == 1);
        let mut symbols: HashMap<u32, SymbolKind> = HashMap::new();
        let mut func_indices: HashMap<u32, u32> = HashMap::new();
        while !reader.eof() {
            let table_code = reader.read_var_u32().unwrap();
            let table = reader.read_string().unwrap();
            if table_code == 0x8 /* WASM_SYMBOL_TABLE */ {
                let mut table_reader = BinaryReader::new(table);
                let table_len = table_reader.read_var_u32().unwrap();
                for index in 0..table_len {
                    let symbol_kind = table_reader.read_var_u32().unwrap();
                    let symbol_flags = table_reader.read_var_u32().unwrap();
                    let wasm_symbol_undefined_flag = 0x10; /* _WASM_SYMBOL_UNDEFINED */
                    // println!("k{}", symbol_kind);
                    match symbol_kind {
                        0x0 /* WASM_SYMBOL_TYPE_FUNCTION */ => {
                            let elem_index = table_reader.read_var_u32().unwrap();
                            if (symbol_flags & wasm_symbol_undefined_flag) == 0 {
                                table_reader.read_string().unwrap();
                            }
                            func_indices.insert(index as u32, elem_index);
                        }
                        0x2 /* WASM_SYMBOL_TYPE_GLOBAL */ => {
                            table_reader.read_var_u32().unwrap();
                            if (symbol_flags & wasm_symbol_undefined_flag) == 0 {
                                table_reader.read_string().unwrap();
                            }
                        }
                        0x1 /* WASM_SYMBOL_TYPE_DATA */ => {
                            table_reader.read_string().unwrap();
                            if (symbol_flags & wasm_symbol_undefined_flag) == 0 {
                                let data_segment = table_reader.read_var_u32().unwrap();
                                let offset = table_reader.read_var_u32().unwrap();
                                table_reader.read_var_u32().unwrap();

                                symbols.insert(index as u32, SymbolKind::Data(data_segment, offset));
                            }
                        }
                        0x3 /* WASM_SYMBOL_TYPE_SECTION */ => {
                            let section_index = table_reader.read_var_u32().unwrap();
                            symbols.insert(index as u32, SymbolKind::Section(section_index));
                        }
                        _ => panic!("unknown symbol kind")
                    }
                }
            }
        }
        (func_indices, symbols)
    };

    let reloc_tables_names = {
        let mut reloc_tables_names = Vec::new();
        for key in debug_sections.reloc_tables.keys() {
            reloc_tables_names.push(key.clone());
        }
        reloc_tables_names
    };

    for ref reloc_table_name in reloc_tables_names {
        let reloc_table = debug_sections.reloc_tables[reloc_table_name].clone();
        let fixup_section_name = &reloc_table_name[6..];
        let mut reader = BinaryReader::new(&reloc_table);
        reader.read_var_u32().unwrap(); // TODO use section_index
        let count = reader.read_var_u32().unwrap();
        for _ in 0..count {
            let ty = reader.read_var_u32().unwrap();
            let mut table_entry = debug_sections.tables.get_mut(&to_vec(fixup_section_name));
            let table: &mut Vec<u8> = table_entry.unwrap();
            let fixup_offset = reader.read_var_u32().unwrap() as usize;

            let index = reader.read_var_u32().unwrap();
            let target_offset = match ty {
                5 => {
                    if let SymbolKind::Data(segment, offset) = symbols[&index] {
                        debug_sections.data_segment_offsets[segment as usize] + offset
                    } else {
                        panic!("unexpected symbol type");
                    }
                }
                8 => {
                    let func_index = func_indices.get(&index).unwrap();
                    debug_sections.func_offsets[*func_index as usize] as u32 // function offset
                }
                9 => 0, // section offset,
                _ => panic!("unexpected reloc type")
            };

            let target_addend = reader.read_var_u32().unwrap();

            let _old_offset = (table[fixup_offset + 0] as u32) |
              ((table[fixup_offset + 1] as u32) << 8) |
              ((table[fixup_offset + 2] as u32) << 16) |
              ((table[fixup_offset + 3] as u32) << 24);

            let offset = target_offset + target_addend;
            table[fixup_offset + 0] = (offset & 0xFF) as u8;
            table[fixup_offset + 1] = ((offset >> 8) & 0xFF) as u8;
            table[fixup_offset + 2] = ((offset >> 16) & 0xFF) as u8;
            table[fixup_offset + 3] = ((offset >> 24) & 0xFF) as u8;

            // println!("{:x}: {:x} -> {:x} ({} {} t{})",
            //   fixup_offset, _old_offset, offset,
            //   str::from_utf8(fixup_section_name).unwrap(), index, ty);
        }
    }
}