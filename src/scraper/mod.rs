use std::{time::Duration};

use deadpool_postgres::{GenericClient, Pool};
use reqwest::Client;
use tap::TapFallible;
use self::{
    db_models::Server,
    errors::SearchError,
    models::{PublicRooms, ServerWellKnown},
};

pub mod db_models;
pub mod errors;
pub mod models;
pub struct Finder {
    postgres: Pool,
    reqwest: Client,
}

impl Finder {
    pub fn new(postgres: Pool) -> Self {
        Finder { postgres, reqwest: reqwest::Client::new() }
    }

    pub async fn init_db(&self) {
        let conn = &self.postgres.get().await.unwrap();
        conn.execute("CREATE TABLE IF NOT EXISTS servers (host text PRIMARY KEY, last_tried BIGINT, last_error text, blacklist BOOLEAN NOT NULL)", &[])
            .await
            .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rooms (id text NOT NULL, server text NOT NULL, alias text, title text, topic text, avatar text, members integer NOT NULL, PRIMARY KEY (id, server))",
            &[],
        )
        .await
        .unwrap();
    }

    pub async fn search(&self, start: &str) -> Result<(), SearchError> {
        let conn = self.postgres.get().await?;
        let mut servers = Vec::<Server>::new();
        for ele in conn.query("SELECT * FROM servers", &[]).await.tap_err(|e| println!("{e:#?}"))? {
            let server = Server {
                blacklist: ele.get("blacklist"),
                host: ele.get("host"),
                last_error: ele.get("last_error"),
                last_tried: ele.get("last_tried"),
            };
            servers.push(server);
        }

        if servers.is_empty() {
            servers.push(Server {
                host: start.to_string(),
                last_tried: None,
                last_error: None,
                blacklist: false,
            });
        }

        // let's avoid putting too much pressure on servers by requesting at most 50 rooms every 1 minute
        // also avoid putting too much pressure on ourselves and spawn a task every 30s
        let mut vec = Vec::new();
        for host in servers {
            vec.push(tokio::spawn(Self::add_from_server(self.reqwest.clone(), self.postgres.clone(), host.host)));
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        for ele in vec {
            ele.await.unwrap().tap_err(|e| log::error!("{:#?}", e));
        }

        Ok(())
    }

    async fn add_from_server(reqwest: Client, postgres: Pool, server: String) -> Result<(), SearchError> {
        let server_url = Self::parse_wellknown(&reqwest, &server).await.unwrap_or(server.clone());
        let mut conn = postgres.get().await?;
        let mut trans = conn.transaction().await?;
        trans.execute("DELETE FROM rooms WHERE server = $1", &[&server]).await.tap_err(|e| log::error!("{:#?}", e))?;

        let mut next_batch = Option::<String>::None;
        let mut i = 0;
        loop {
            let query;
            if next_batch.is_none() {
                query = format!("https://{server_url}/_matrix/client/v3/publicRooms?limit=5000");
            } else {
                query = format!("https://{server_url}/_matrix/client/v3/publicRooms?limit=5000&since={}", urlencoding::encode(next_batch.unwrap().as_str()));
            }

            let build = reqwest.get(query);
            let sent = build.send().await?;
            let text = sent.text().await?;
            let parsed = serde_json::from_str::<PublicRooms>(text.as_str()).tap_err(|e| log::error!("Error: {e:#?}, string: {text}"))?;

            for room in parsed.chunk {
                trans
                    .execute(
                        "INSERT INTO rooms (id, server, alias, title, topic, avatar, members) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                        &[&room.room_id, &server, &room.canonical_alias, &room.name, &room.topic, &room.avatar_url, &room.num_joined_members],
                    )
                    .await
                    .tap_err(|e| log::error!("{:#?}", e))?;

                if let Some(link) = room.canonical_alias {
                    let hostname = Self::get_hostname(&link);
                    if hostname.is_none() {
                        continue;
                    }

                    let res = trans
                        .execute(
                            "INSERT INTO servers (host, last_tried, last_error, blacklist) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                            &[&hostname, &None::<i64>, &None::<String>, &false],
                        )
                        .await
                        .tap_err(|e| log::error!("{e:#?}"));
                }
            }

            if parsed.next_batch.is_none() {
                println!("My job here is done");
                break;
            } else {
                next_batch = parsed.next_batch;
            }

            println!("Waiting another 60secs");
        }

        trans.commit().await?;
        Ok(())
    }

    async fn parse_wellknown(reqwest: &Client, host: &String) -> Option<String> {
        let build = reqwest.get(format!("https://{host}/.well-known/matrix/server"));
        let sent = build.send().await;
        if sent.is_err() {
            return None;
        }

        let text = sent.unwrap().text().await;
        if text.is_err() {
            return None;
        }

        let parsed = serde_json::from_str::<ServerWellKnown>(text.unwrap().as_str());
        if parsed.is_err() {
            return None;
        }

        parsed.unwrap().server
    }

    fn get_hostname(host: &String) -> Option<&str> {
        if let Some(hostname_pos) = host.find(':') {
            if hostname_pos + 1 > host.len() {
                log::warn!("An invalid hostname was provided. Hostname {}", host);
                return None;
            }

            return Some(&host[(hostname_pos + 1)..host.len()]);
        }

        None
    }
}
