// Reads wasm file debug sections contents.

use std::collections::HashMap;

use wasmparser::{
    Parser,WasmDecoder,ParserState,ParserInput,SectionCode,
    ImportSectionEntryType,Operator
};

fn is_reloc_debug_section_name(name: &[u8]) -> bool {
  return name.len() >= 13 && &name[0..13] == b"reloc..debug_" ;
}

fn is_debug_section_name(name: &[u8]) -> bool {
    return name.len() >= 7 && &name[0..7] == b".debug_";
}

fn is_linking_section_name(name: &[u8]) -> bool {
    return name == b"linking";
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
                    code: SectionCode::Custom {
                        ref name,
                        ..
                    },
                    .. 
                } if is_debug_section_name(name) ||
                     is_reloc_debug_section_name(name) ||
                     is_linking_section_name(name) => {
                    let mut name_copy = Vec::new();
                    name_copy.extend_from_slice(name);
                    current_section_name = Some(name_copy);
                    input = ParserInput::ReadSectionRawData;
                },
                ParserState::SectionRawData(ref data) => {
                    let mut data_copy = Vec::new();
                    data_copy.extend_from_slice(data);

                    let mut name_copy = Vec::new();
                    name_copy.extend_from_slice(current_section_name.as_ref().unwrap());
                    tables_index.insert(section_index, name_copy);

                    let section_name = current_section_name.take().unwrap();
                    if is_debug_section_name(&section_name) {          
                        tables.insert(
                            section_name,
                            data_copy
                        );
                    } else if is_reloc_debug_section_name(&section_name) {
                        reloc_tables.insert(
                            section_name,
                            data_copy
                        );
                    } else {
                        assert!(is_linking_section_name(&section_name));
                        linking = Some(data_copy);
                    }

                    input = ParserInput::Default;
                },
                ParserState::BeginSection { code: SectionCode::Import, .. } |
                ParserState::BeginSection { code: SectionCode::Data, .. } |
                ParserState::BeginSection { code: SectionCode::Code, .. } => {
                    input = ParserInput::Default;
                },
                ParserState::BeginFunctionBody { .. } => {
                    if code_content.is_none() {
                        code_content = Some(offset);
                    }
                    input = ParserInput::SkipFunctionBody;
                },
                ParserState::ImportSectionEntry { ty: ImportSectionEntryType::Function(..), .. } => {
                    func_offsets.push(0); // include imports?
                    input = ParserInput::Default;
                }
                ParserState::BeginSection { .. } => {
                    input = ParserInput::SkipSection;
                },
                ParserState::EndSection => {
                    section_index += 1;
                    input = ParserInput::Default;
                },
                ParserState::InitExpressionOperator(ref op) => {
                    if let Operator::I32Const { value, } = op {
                        data_segment_offsets.push(*value as u32);
                    } else {
                        panic!("Unexpected init expression operator");
                    }
                    input = ParserInput::Default;
                },
                _ => {
                    input = ParserInput::Default;
                },
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