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
    description: "Query recent messages across ALL platforms — WhatsApp, Telegram, Signal, AND EMAIL. \
        Use this tool for ANY question about the user's messages, chats, inbox, emails, digests, or \
        'what came in'. Pass platform='email' to see only emails, or omit platform to see everything. \
        NEVER answer from conversation history alone — always call this tool fresh, even if a prior \
        assistant turn in history already contained a digest (that history is stale and may be hours old). \
        Results are sorted most-recent-first and include a timestamp per row. If the default limit \
        doesn't reach far enough back for the user's question (e.g. 'yesterday', 'past few days'), \
        call again with a larger `limit`. Each result includes a stable [id=N] you MUST cite in your \
        answer — the caller uses it to verify nothing was fabricated.",
    properties: MESSAGE_PROPS,
    linkable_to: &[],
};

static EVENT_PROPS: &[PropertyDef] = &[PropertyDef {
    name: "status",
    description: "Filter by obligation status. Omit to show only active obligations.",
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
    description: "Query tracked obligations. Use for specific commitments with a due time and reminder time, not umbrella situations.",
    properties: EVENT_PROPS,
    linkable_to: &[],
};

// ---------------------------------------------------------------------------
// Registry implementation
// ---------------------------------------------------------------------------

impl OntologyRegistry {
    pub fn build() -> Self {
        Self {
            object_types: vec![&PERSON_DEF, &MESSAGE_DEF, &EVENT_DEF],
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

        // Message-specific: let the LLM control how many of the most
        // recent messages it wants back. Single dial — no separate time
        // window — matches Palantir's Object query tool pattern. If the
        // result ends before the user's desired range, the LLM can call
        // again with a bigger limit.
        if obj.name == "Message" {
            properties.insert(
                "limit".to_string(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(types::JSONSchemaType::Number),
                    description: Some(
                        "Max messages to return, most recent first (default 20, max 100). \
                         Increase when the result doesn't reach far enough back for the \
                         user's question (e.g. 'yesterday' or 'past few days'). Each result \
                         has a timestamp so you can tell whether to request more."
                            .to_string(),
                    ),
                    ..Default::default()
                }),
            );
        }

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
