use std::{thread, time::Duration};

use tokio::sync::broadcast;

use crate::proto::Event;

use super::Database;

pub fn start_poller(database: Database, events: broadcast::Sender<Event>, pull_delay: u64) {
    thread::spawn(move || {
        let mut last_database_id = database.latest_message_database_id();

        loop {
            thread::sleep(Duration::from_millis(pull_delay));
            let rows = match database.messages_after(last_database_id, 100) {
                Ok(rows) => rows,
                Err(error) => {
                    eprintln!("message polling failed: {error}");
                    continue;
                }
            };

            for row in rows {
                last_database_id = last_database_id.max(row.database_id);
                match database.map_message(row) {
                    Ok(event) => {
                        let _ = events.send(event);
                    }
                    Err(error) => eprintln!("message mapping failed: {error}"),
                }
            }
        }
    });
}
