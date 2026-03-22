use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;

/// Where a filterable property gets its enum values from.
pub enum FilterSource {
    /// Enum values known at compile time (e.g., platform types).
    Static(&'static [&'static str]),
    /// Enum values loaded from DB per user (key into OntologyUserData.dynamic_enums).
    Dynamic(&'static str),
}

pub struct PropertyDef {
    pub name: &'static str,
    pub description: &'static str,
    pub prop_type: &'static str,
    /// None = not a tool parameter, Some = becomes a filterable tool param.
    pub filter: Option<FilterSource>,
}

pub struct ObjectTypeDef {
    pub name: &'static str,
    pub description: &'static str,
    pub properties: &'static [PropertyDef],
    pub linkable_to: &'static [&'static str],
}

/// User-specific data injected into tool definitions at call time.
pub struct OntologyUserData {
    pub dynamic_enums: HashMap<String, Vec<String>>,
}

pub struct OntologyRegistry {
    pub object_types: Vec<&'static ObjectTypeDef>,
}

// ---------------------------------------------------------------------------
// Static object type definitions
// ---------------------------------------------------------------------------

static PERSON_PROPS: &[PropertyDef] = &[
    PropertyDef {
        name: "name",
        description: "Select a person by name, or 'all' to list everyone.",
        prop_type: "String",
        filter: Some(FilterSource::Dynamic("person_names")),
    },
    PropertyDef {
        name: "created_at",
        description: "When the person was created (unix timestamp).",
        prop_type: "Integer",
        filter: None,
    },
];

static PERSON_DEF: ObjectTypeDef = ObjectTypeDef {
    name: "Person",
    description: "Query people in your contacts. Returns person details with their channels.",
    properties: PERSON_PROPS,
    linkable_to: &["Channel"],
};

static CHANNEL_PROPS: &[PropertyDef] = &[
    PropertyDef {
        name: "platform",
        description: "Filter by platform type, or 'all' for every platform.",
        prop_type: "String",
        filter: Some(FilterSource::Static(&[
            "whatsapp", "telegram", "signal", "email",
        ])),
    },
    PropertyDef {
        name: "person_name",
        description: "Filter channels belonging to a specific person, or 'all'.",
        prop_type: "String",
        filter: Some(FilterSource::Dynamic("person_names")),
    },
    PropertyDef {
        name: "handle",
        description: "The contact handle (phone number, email address, etc.).",
        prop_type: "Optional<String>",
        filter: None,
    },
    PropertyDef {
        name: "room_id",
        description: "The Matrix room ID for this channel.",
        prop_type: "Optional<String>",
        filter: None,
    },
    PropertyDef {
        name: "notification_mode",
        description: "Filter by notification mode, or 'all'.",
        prop_type: "String",
        filter: Some(FilterSource::Static(&["alert", "silent", "off"])),
    },
];

static CHANNEL_DEF: ObjectTypeDef = ObjectTypeDef {
    name: "Channel",
    description: "Query communication channels. Returns channels with their parent person.",
    properties: CHANNEL_PROPS,
    linkable_to: &["Person"],
};

static MESSAGE_PROPS: &[PropertyDef] = &[
    PropertyDef {
        name: "platform",
        description: "Filter by platform. Omit to include all platforms.",
        prop_type: "String",
        filter: Some(FilterSource::Static(&[
            "whatsapp", "telegram", "signal", "email",
        ])),
    },
    PropertyDef {
        name: "sender_name",
        description: "Filter by sender name. Omit to include all senders.",
        prop_type: "String",
        filter: Some(FilterSource::Dynamic("person_names")),
    },
];

static MESSAGE_DEF: ObjectTypeDef = ObjectTypeDef {
    name: "Message",
    description: "Query recent messages. All filters are optional - omit to get recent messages.",
    properties: MESSAGE_PROPS,
    linkable_to: &[],
};

static EVENT_PROPS: &[PropertyDef] = &[PropertyDef {
    name: "status",
    description: "Filter by event status. Omit to show only active events.",
    prop_type: "String",
    filter: Some(FilterSource::Static(&[
        "active",
        "notified",
        "expired",
        "dismissed",
    ])),
}];

static EVENT_DEF: ObjectTypeDef = ObjectTypeDef {
    name: "Event",
    description: "Query tracked events. Shows active events with IDs, descriptions, and deadlines.",
    properties: EVENT_PROPS,
    linkable_to: &[],
};

// ---------------------------------------------------------------------------
// Registry implementation
// ---------------------------------------------------------------------------

impl OntologyRegistry {
    pub fn build() -> Self {
        Self {
            object_types: vec![&PERSON_DEF, &CHANNEL_DEF, &MESSAGE_DEF, &EVENT_DEF],
        }
    }

    /// Build one tool definition per entity type, injecting user-specific enum values.
    pub fn build_query_tools(&self, user_data: &OntologyUserData) -> Vec<chat_completion::Tool> {
        self.object_types
            .iter()
            .map(|obj| self.build_tool_for_type(obj, user_data))
            .collect()
    }

    fn build_tool_for_type(
        &self,
        obj: &ObjectTypeDef,
        user_data: &OntologyUserData,
    ) -> chat_completion::Tool {
        let tool_name = format!("query_{}", obj.name.to_lowercase());
        let mut properties = HashMap::new();

        // Add per-property filter params
        for prop in obj.properties.iter() {
            if let Some(ref filter) = prop.filter {
                let mut enum_values = vec!["all".to_string()];
                match filter {
                    FilterSource::Static(vals) => {
                        enum_values.extend(vals.iter().map(|v| v.to_string()));
                    }
                    FilterSource::Dynamic(key) => {
                        if let Some(vals) = user_data.dynamic_enums.get(*key) {
                            enum_values.extend(vals.clone());
                        }
                    }
                }

                properties.insert(
                    prop.name.to_string(),
                    Box::new(types::JSONSchemaDefine {
                        schema_type: Some(types::JSONSchemaType::String),
                        description: Some(prop.description.to_string()),
                        enum_values: Some(enum_values),
                        ..Default::default()
                    }),
                );
            }
        }

        // Always add free-text "query" param
        properties.insert(
            "query".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(format!(
                    "Free-text keyword search across {} properties.",
                    obj.name.to_lowercase()
                )),
                ..Default::default()
            }),
        );

        // Add "linked_entities" array param from linkable_to
        if !obj.linkable_to.is_empty() {
            let linkable_enum: Vec<String> =
                obj.linkable_to.iter().map(|s| s.to_string()).collect();

            properties.insert(
                "linked_entities".to_string(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(types::JSONSchemaType::Array),
                    description: Some("Include linked entities in results.".to_string()),
                    items: Some(Box::new(types::JSONSchemaDefine {
                        schema_type: Some(types::JSONSchemaType::String),
                        enum_values: Some(linkable_enum),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
            );
        }

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: tool_name,
                description: Some(obj.description.to_string()),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: None,
                },
            },
        }
    }
}
