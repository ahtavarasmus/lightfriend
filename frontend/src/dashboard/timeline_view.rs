#[derive(Clone, PartialEq)]
pub struct UpcomingItem {
    pub item_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub description: String,
    pub date_display: String,
    pub relative_display: String,
    pub item_type: Option<String>,
    pub notify: Option<String>,
    pub sources_display: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct UpcomingDigest {
    pub item_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub sources: Option<String>,
}
