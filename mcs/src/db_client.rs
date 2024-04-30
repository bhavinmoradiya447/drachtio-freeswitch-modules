use sqlite::{Connection, ConnectionThreadSafe};
use tracing::{error, info}

#[derive(Debug, Default)]
pub struct CallDetails {
    pub call_leg_id: String,
    pub client_address: String,
    pub codec: String,
    pub mode: String,
    pub metadata: String,
}

pub struct DbClient {
    connection: ConnectionThreadSafe,
}

impl DbClient {
    pub fn new() -> Self {
        let connection = Connection::open_thread_safe("call-details-db").unwrap();
        let query = "
            BEGIN;
            CREATE TABLE IF NOT EXISTS CALL_DETAILS (
            CALL_LEG_ID VARCHAR(36),
            CLIENT_ADDRESS VARCHAR(256),
            CODEC VARCHAR(64),
            MODE VARCHAR(16),
            META_DATA TEXT,
            PRIMARY KEY (CALL_LEG_ID, CLIENT_ADDRESS)
            );
            CREATE INDEX IF NOT EXISTS  call_leg_id_idx ON CALL_DETAILS (CALL_LEG_ID);
            COMMIT;";
        connection.execute(query).unwrap();
        Self { connection }
    }

    pub fn insert(&self, call_details: CallDetails) {
        let query = format!("INSERT into CALL_DETAILS values(\"{}\",\"{}\",\"{}\",\"{}\",\"{}\")",
                            call_details.call_leg_id,
                            call_details.client_address, call_details.codec, call_details.mode, call_details.metadata);
        match self.connection.execute(query)
        {
            Ok(()) => info!("Successfully inserted {} , {}", call_details.call_leg_id, call_details.client_address),
            Err(e) => error!("Failed to insert {} , {}", call_details.call_leg_id, call_details.client_address)
        }
    }

    pub fn delete_by_call_leg_and_client_address(&self, call_leg_id: String, client_address: String) {
        let query = format!("DELETE from CALL_DETAILS where CALL_LEG_ID=\"{}\" and CLIENT_ADDRESS=\"{}\"",
                            call_leg_id,
                            client_address);
        self.connection.execute(query).unwrap();
    }

    pub fn delete_by_call_leg_id(&self, call_leg_id: String) {
        let query = format!("DELETE from CALL_DETAILS where CALL_LEG_ID=\"{}\"",
                            call_leg_id);
        self.connection.execute(query).unwrap();
    }

    pub fn select_all(&self) -> Vec<CallDetails> {
        let mut call_details = vec![];


        let query = "SELECT * FROM CALL_DETAILS";

        self.connection
            .iterate(query, |pairs| {
                call_details.push(CallDetails {
                    call_leg_id: pairs.get(0).unwrap().1.unwrap().parse().unwrap(),
                    client_address: pairs.get(1).unwrap().1.unwrap().parse().unwrap(),
                    codec: pairs.get(2).unwrap().1.unwrap().parse().unwrap(),
                    mode: pairs.get(3).unwrap().1.unwrap().parse().unwrap(),
                    metadata: pairs.get(4).unwrap().1.unwrap().parse().unwrap(),
                });
                true
            })
            .unwrap();
        call_details
    }
}