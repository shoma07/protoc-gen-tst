extern crate protobuf;

use protobuf::parse_from_reader;
use protobuf::plugin::*;
use protobuf::descriptor::*;
use protobuf::error::ProtobufResult;
use protobuf::Message;
use std::io::stdin;
use std::io::stdout;
use std::fmt;

enum TsType {
    Boolean,
    Number,
    String,
    Never,
    Object(String)
}

impl fmt::Display for TsType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TsType::Boolean => write!(f, "boolean"),
            TsType::Number => write!(f, "number"),
            TsType::String => write!(f, "string"),
            TsType::Never => write!(f, "never"),
            TsType::Object(name) => write!(f, "{}", name)
        }
    }
}

fn field_type_to_ts_type(field: &FieldDescriptorProto) -> TsType {
    match field.get_field_type() {
        FieldDescriptorProto_Type::TYPE_DOUBLE |
            FieldDescriptorProto_Type::TYPE_FLOAT |
            FieldDescriptorProto_Type::TYPE_INT64 |
            FieldDescriptorProto_Type::TYPE_UINT64 |
            FieldDescriptorProto_Type::TYPE_INT32 |
            FieldDescriptorProto_Type::TYPE_FIXED64 |
            FieldDescriptorProto_Type::TYPE_FIXED32 |
            FieldDescriptorProto_Type::TYPE_UINT32 |
            FieldDescriptorProto_Type::TYPE_SFIXED32 |
            FieldDescriptorProto_Type::TYPE_SFIXED64 |
            FieldDescriptorProto_Type::TYPE_SINT32 |
            FieldDescriptorProto_Type::TYPE_SINT64 => TsType::Number,
            FieldDescriptorProto_Type::TYPE_STRING |
                FieldDescriptorProto_Type::TYPE_BYTES => TsType::String,
            FieldDescriptorProto_Type::TYPE_BOOL => TsType::Boolean,
            FieldDescriptorProto_Type::TYPE_ENUM |
                FieldDescriptorProto_Type::TYPE_MESSAGE |
                FieldDescriptorProto_Type::TYPE_GROUP => TsType::Object(field.get_type_name().to_string())
    }
}

enum TsFieldType {
    Single(TsType),
    Array(TsType)
}

impl fmt::Display for TsFieldType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TsFieldType::Single(ts_type) => write!(f, "{}", ts_type),
            TsFieldType::Array(ts_type) => write!(f, "ReadonlyArray<{}>", ts_type)
        }
    }
}

struct TsField {
    key: String,
    ts_type: TsFieldType,
    is_required: bool
}

impl fmt::Display for TsField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.is_required {
            true => write!(f, "{}: {};\n", self.key, self.ts_type),
            false => write!(f, "{}?: {};\n", self.key, self.ts_type)
        }
    }
}

struct TsObjectType {
    name: String,
    fields: Vec<TsField>,
    oneof_list: Vec<Vec<TsField>>
}

impl fmt::Display for TsObjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let oneof_list_len = self.oneof_list.len();
        let fields_len = self.fields.len();
        write!(f, "type {} = ", self.name)?;
        if fields_len > 0 {
            write!(f, "Readonly<{{\n")?;
        }
        self.fields.iter().for_each(|field| {
            write!(f, "  {}", field);
        });
        if fields_len > 0 {
            write!(f, "}}>")?;
            if oneof_list_len > 0 { write!(f, " & ")?; }
        }
        for (i, oneof) in self.oneof_list.iter().enumerate() {
            let oneof_last_index = oneof.len() - 1;
            write!(f, "Readonly<\n")?;
            for (j, field_i) in oneof.iter().enumerate() {
                write!(f, "    {{\n");
                oneof.iter().for_each(|field_j| {
                    if field_i.key == field_j.key {
                        write!(f, "      {}", field_j);
                    } else {
                        write!(
                            f,
                            "      {}",
                            TsField{
                                key: field_j.key.clone(),
                                ts_type: TsFieldType::Single(TsType::Never),
                                is_required: field_j.is_required
                            }
                        );
                    }
                });
                write!(f, "    }}")?;
                if j < oneof_last_index { write!(f, " |")?; }
                write!(f, "\n")?;
            }
            write!(f, "  >")?;
            if i < oneof_list_len - 1 { write!(f, " & ")?; }
        }
        write!(f, ";\n")?;
        Ok(())
    }
}

fn process_req(req: CodeGeneratorRequest) -> ProtobufResult<CodeGeneratorResponse> {
    let mut resp = CodeGeneratorResponse::new();
    resp.set_file(
        req.get_proto_file().iter().map(|proto_file|
            proto_file.get_message_type().iter().map(|message_type| {
                let mut oneof_list = Vec::<Vec::<TsField>>::new();
                message_type.get_oneof_decl().iter().for_each(|_i| {
                    oneof_list.push(Vec::<TsField>::new());
                });
                message_type.get_field()
                    .iter()
                    .filter(|field| field.has_oneof_index())
                    .for_each(|field| {
                        oneof_list[field.get_oneof_index() as usize].push(TsField{
                            key: field.get_json_name().to_string(),
                            ts_type: match field.get_label() {
                                FieldDescriptorProto_Label::LABEL_OPTIONAL |
                                    FieldDescriptorProto_Label::LABEL_REQUIRED =>
                                    TsFieldType::Single(field_type_to_ts_type(&field)),
                                FieldDescriptorProto_Label::LABEL_REPEATED =>
                                    TsFieldType::Array(field_type_to_ts_type(&field))
                            },
                            is_required: false
                        })
                    });
                let ts_object_type = TsObjectType{
                    name: message_type.get_name().to_string(),
                    fields: message_type.get_field()
                        .iter()
                        .filter(|field| !field.has_oneof_index())
                        .map(|field|
                            TsField{
                                key: field.get_json_name().to_string(),
                                ts_type: match field.get_label() {
                                    FieldDescriptorProto_Label::LABEL_OPTIONAL |
                                        FieldDescriptorProto_Label::LABEL_REQUIRED =>
                                        TsFieldType::Single(field_type_to_ts_type(&field)),
                                    FieldDescriptorProto_Label::LABEL_REPEATED =>
                                        TsFieldType::Array(field_type_to_ts_type(&field))
                                },
                                is_required: true
                            }
                        ).collect(),
                        oneof_list: oneof_list
                };
                gen_resp_file(
                    ts_object_type.name.clone(),
                    format!("{}", ts_object_type)
                )
            })
        ).flatten().collect()
    );
    Ok(resp)
}

fn gen_resp_file(name: String, content: String) -> CodeGeneratorResponse_File {
    let mut file = CodeGeneratorResponse_File::new();
    file.set_name(name + ".d.ts");
    file.set_content(content);
    file
}

fn main() {
    process_req(
        parse_from_reader::<CodeGeneratorRequest>(&mut stdin()).unwrap()
    ).unwrap().write_to_writer(&mut stdout()).unwrap();
}
