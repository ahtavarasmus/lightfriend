#[derive(Clone, PartialEq)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String,
    pub summary: String,
    pub description: String,
    pub priority: i32,
    pub due_at: Option<i32>,
    pub source: Option<String>,
    pub source_id: Option<String>,
    pub notify: Option<String>,
    pub sender: Option<String>,
    pub platform: Option<String>,
    pub time_display: Option<String>,
    pub relative_display: Option<String>,
}
