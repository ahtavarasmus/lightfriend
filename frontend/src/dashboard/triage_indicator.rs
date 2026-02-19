#[derive(Clone, PartialEq)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String,
    pub summary: String,
    pub next_check_at: Option<i32>,
    pub source: Option<String>,
    pub source_id: Option<String>,
}
