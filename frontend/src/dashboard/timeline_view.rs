#[derive(Clone, PartialEq)]
pub struct UpcomingTask {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub description: String,
    pub date_display: String,
    pub relative_display: String,
    pub condition: Option<String>,
    pub sources_display: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct UpcomingDigest {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub sources: Option<String>,
}
