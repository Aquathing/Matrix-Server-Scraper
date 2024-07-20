pub struct Server {
    pub host: String,
    pub last_tried: Option<i64>,
    pub last_error: Option<String>,
    pub blacklist: bool,
}
