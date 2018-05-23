// Reads wasm file debug sections contents.

use std::collections::HashMap;
use std::io::Write;

use wasmparser::{ImportSectionEntryType, Operator, Parser, ParserInput, ParserState, SectionCode,
                 WasmDecoder};

fn is_reloc_debug_section_name(name: &[u8]) -> bool {
    return name.len() >= 13 && &name[0..13] == b"reloc..debug_";
}

fn is_debug_section_name(name: &[u8]) -> bool {
    return name.len() >= 7 && &name[0..7] == b".debug_";
}

fn is_linking_section_name(name: &[u8]) -> bool {
    return name == b"linking";
}

fn is_source_mapping_section_name(name: &[u8]) -> bool {
    return name == b"sourceMappingURL";
}

pub struct DebugSections {
    pub tables: HashMap<Vec<u8>, Vec<u8>>,
    pub tables_index: HashMap<usize, Vec<u8>>,
    pub reloc_tables: HashMap<Vec<u8>, Vec<u8>>,
    pub linking: Option<Vec<u8>>,
    pub code_content: usize,
    pub func_offsets: Vec<usize>,
    pub data_segment_offsets: Vec<u32>,
}

impl DebugSections {
    pub fn read_sections(wasm: &[u8]) -> DebugSections {
        let mut parser = Parser::new(wasm);
        let mut input = ParserInput::Default;
        let mut current_section_name = None;
        let mut linking: Option<Vec<u8>> = None;
        let mut tables = HashMap::new();
        let mut tables_index = HashMap::new();
        let mut reloc_tables = HashMap::new();
        let mut code_content: Option<usize> = None;
        let mut func_offsets = Vec::new();
        let mut data_segment_offsets = Vec::new();
        let mut section_index = 0;
        let mut data_copy = None;

        loop {
            let offset = parser.current_position();
            if let ParserInput::SkipFunctionBody = input {
                // Saw function body start, saving as func_offset
                func_offsets.push(offset - code_content.unwrap());
            }

            let state = parser.read_with_input(input);
            match *state {
                ParserState::EndWasm => break,
                ParserState::Error(err) => panic!("Error: {:?}", err),
                ParserState::BeginSection {
                    code: SectionCode::Custom { ref name, .. },
                    ..
                } if is_debug_section_name(name) || is_reloc_debug_section_name(name)
                    || is_linking_section_name(name)
                    || is_source_mapping_section_name(name) =>
                {
                    let mut name_copy = Vec::new();
                    name_copy.extend_from_slice(name);
                    current_section_name = Some(name_copy);
                    data_copy = Some(Vec::new());
                    input = ParserInput::ReadSectionRawData;
                }
                ParserState::SectionRawData(ref data) => {
                    data_copy.as_mut().unwrap().extend_from_slice(data);
                    input = ParserInput::Default;
                }
                ParserState::BeginSection {
                    code: SectionCode::Import,
                    ..
                }
                | ParserState::BeginSection {
                    code: SectionCode::Data,
                    ..
                }
                | ParserState::BeginSection {
                    code: SectionCode::Code,
                    ..
                } => {
                    input = ParserInput::Default;
                }
                ParserState::BeginFunctionBody { .. } => {
                    if code_content.is_none() {
                        code_content = Some(offset);
                    }
                    input = ParserInput::SkipFunctionBody;
                }
                ParserState::ImportSectionEntry {
                    ty: ImportSectionEntryType::Function(..),
                    ..
                } => {
                    func_offsets.push(0); // include imports?
                    input = ParserInput::Default;
                }
                ParserState::BeginSection { .. } => {
                    input = ParserInput::SkipSection;
                }
                ParserState::EndSection => {
                    section_index += 1;

                    if data_copy.is_some() {
                        let mut name_copy = Vec::new();
                        name_copy.extend_from_slice(current_section_name.as_ref().unwrap());
                        tables_index.insert(section_index, name_copy);

                        let section_name = current_section_name.take().unwrap();
                        let data = data_copy.take().unwrap();
                        if is_debug_section_name(&section_name) {
                            tables.insert(section_name, data);
                        } else if is_reloc_debug_section_name(&section_name) {
                            reloc_tables.insert(section_name, data);
                        } else {
                            assert!(is_linking_section_name(&section_name));
                            linking = Some(data);
                        }
                    }
                    input = ParserInput::Default;
                }
                ParserState::InitExpressionOperator(ref op) => {
                    if let Operator::I32Const { value } = op {
                        data_segment_offsets.push(*value as u32);
                    } else {
                        panic!("Unexpected init expression operator");
                    }
                    input = ParserInput::Default;
                }
                _ => {
                    input = ParserInput::Default;
                }
            }
        }
        DebugSections {
            tables,
            tables_index,
            reloc_tables,
            linking,
            code_content: code_content.unwrap(),
            func_offsets,
            data_segment_offsets,
        }
    }
}

pub fn remove_debug_sections(wasm: &[u8], write: &mut Write) {
    let mut parser = Parser::new(wasm);
    let mut input = ParserInput::Default;
    let mut last_written = 0;
    let mut skipping_section = false;
    loop {
        let offset = parser.current_position();
        let state = parser.read_with_input(input);
        match *state {
            ParserState::EndWasm => break,
            ParserState::Error(err) => panic!("Error: {:?}", err),
            ParserState::BeginSection {
                code: SectionCode::Custom { ref name, .. },
                ..
            } if is_debug_section_name(name) || is_reloc_debug_section_name(name)
                || is_linking_section_name(name) =>
            {
                if !skipping_section {
                    write
                        .write(&wasm[last_written..offset])
                        .expect("wasm result written");
                    skipping_section = true;
                }
                input = ParserInput::ReadSectionRawData;
            }
            ParserState::BeginSection { .. } => {
                skipping_section = false;
                input = ParserInput::ReadSectionRawData;
            }
            ParserState::SectionRawData(..) => {
                input = ParserInput::Default;
            }
            ParserState::EndSection => {
                if skipping_section {
                    last_written = offset;
                }
            }
            _ => {}
        }
    }
    if !skipping_section && last_written < wasm.len() {
        write
            .write(&wasm[last_written..wasm.len()])
            .expect("wasm result written");
    }
}

fn convert_to_leb(n: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut i = n;
    while i > 128 {
        buf.push(0x80 | (n & 0x7f) as u8);
        i = i >> 7;
    }
    buf.push(i as u8);
    buf
}

pub fn add_source_mapping_url_section(url: &str, write: &mut Write) {
    let name = b"sourceMappingURL";
    let mut result = Vec::new();
    let custom_section_id = convert_to_leb(0);
    result.extend_from_slice(&custom_section_id);
    let name_size = convert_to_leb(name.len());
    let url_size = convert_to_leb(url.len());
    let payload_size = convert_to_leb(name_size.len() + name.len() + url_size.len() + url.len());
    result.extend_from_slice(&payload_size);
    result.extend_from_slice(&name_size);
    result.extend_from_slice(name);
    result.extend_from_slice(&url_size);
    result.extend_from_slice(url.as_bytes());
    write.write(&result).expect("wasm result written");
}
