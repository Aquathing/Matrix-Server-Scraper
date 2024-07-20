use serde;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PublicRooms {
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub chunk: Vec<Room>,
}

#[derive(Deserialize, Debug)]
pub struct Room {
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub name: Option<String>,
    pub num_joined_members: i32,
    pub room_id: String,
    pub room_type: Option<String>,
    pub topic: Option<String>,
    pub world_readable: bool,
}

#[derive(Deserialize, Debug)]
pub struct ServerWellKnown {
    #[serde(rename = "m.server")]
    pub server: Option<String>,
}
