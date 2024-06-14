use sqlite::{Connection, ConnectionThreadSafe, State, Value};
use tracing::{error, info};

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
        let mut stmt = self.connection.prepare("INSERT into CALL_DETAILS values(:callId,:address,:codec,:mode,:metadata)").unwrap();

        stmt.bind_iter::<_, (_, Value)>([
            (":callId", call_details.call_leg_id.clone().into()),
            (":address", call_details.client_address.clone().into()),
            (":codec", call_details.codec.clone().into()),
            (":mode", call_details.mode.clone().into()),
            (":metadata", call_details.metadata.clone().into())
        ]).unwrap();
        match stmt.next()
        {
            Ok(state) => info!("Insert entry {} , {} is {:?}", call_details.call_leg_id, call_details.client_address, state),
            Err(e) => error!("Failed to insert {} , {}, Error:  {:?}", call_details.call_leg_id, call_details.client_address, e)
        }
    }

    pub fn delete_by_call_leg_and_client_address(&self, call_leg_id: String, client_address: String) {
        let mut stmt =
            self.connection.prepare("DELETE from CALL_DETAILS where CALL_LEG_ID= :callId and CLIENT_ADDRESS= :address").unwrap();

        stmt.bind_iter::<_, (_, Value)>([(":callId", call_leg_id.clone().into()),
            (":address", client_address.clone().into())]).unwrap();

        match stmt.next() {
            Ok(state) => info!("Deleting entry for {} , {} is {:?}", call_leg_id, client_address, state),
            Err(e) => error!("Failed to delete {} , {}, Error:  {:?}", call_leg_id, client_address, e)
        }
    }

    pub fn delete_by_call_leg_id(&self, call_leg_id: String) {
        let mut stmt =
            self.connection.prepare("DELETE from CALL_DETAILS where CALL_LEG_ID= :callId").unwrap();

        stmt.bind_iter::<_, (_, Value)>([(":callId", call_leg_id.clone().into())]).unwrap();

        match stmt.next() {
            Ok(state) => info!("Deleting entry for {}  is {:?}", call_leg_id, state),
            Err(e) => error!("Failed to delete {} , Error:  {:?}", call_leg_id, e)
        }
    }

    pub fn select_all(&self) -> Vec<CallDetails> {
        let mut call_details = vec![];


        let query = "SELECT * FROM CALL_DETAILS";

        match self.connection
            .iterate(query, |pairs| {
                call_details.push(CallDetails {
                    call_leg_id: pairs.get(0).unwrap().1.unwrap().parse().unwrap(),
                    client_address: pairs.get(1).unwrap().1.unwrap().parse().unwrap(),
                    codec: pairs.get(2).unwrap().1.unwrap().parse().unwrap(),
                    mode: pairs.get(3).unwrap().1.unwrap().parse().unwrap(),
                    metadata: pairs.get(4).unwrap().1.unwrap().parse().unwrap(),
                });
                true
            }) {
            Ok(()) => info!("Successfully executed select query"),
            Err(e) => error!("Failed to execute select, Error:  {:?}", e)
        }

        info!("Select query return {} rows, values: {:?}", call_details.len(), call_details);
        call_details
    }

    pub fn select_by_call_id_and_address(&self, call_leg_id: String, client_address: String) -> Result<CallDetails, String> {
        let mut stmt =
            self.connection.prepare("SELECT * FROM CALL_DETAILS where CALL_LEG_ID= :callId and CLIENT_ADDRESS= :address").unwrap();

        stmt.bind_iter::<_, (_, Value)>([(":callId", call_leg_id.clone().into()),
            (":address", client_address.clone().into())]).unwrap();


        match stmt.next() {
            Ok(State::Row) => {
                Ok(CallDetails {
                    call_leg_id: stmt.read(0).unwrap(),
                    client_address: stmt.read(1).unwrap(),
                    codec: stmt.read(2).unwrap(),
                    mode: stmt.read(3).unwrap(),
                    metadata: stmt.read(4).unwrap(),
                })
            }
            Err(e) => { Err(format!("{:?}, {:?}", e.message, e.code)) }
            _ => {
                info!("Got Other then row for select_by_call_id_and_address  {}, {}", call_leg_id.clone(), client_address.clone());
                Err("Got Other then row".parse().unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db_client::{CallDetails, DbClient};

    #[test]
    fn test_db_operation() {
        let mut db_client = DbClient::new();
        let call_details1 = CallDetails {
            call_leg_id: "leg1".to_string(),
            client_address: "address1".to_string(),
            codec: "codec1".to_string(),
            mode: "mode1".to_string(),
            metadata: "metadata1".to_string(),
        };

        let call_details2 = CallDetails {
            call_leg_id: "leg2".to_string(),
            client_address: "address2".to_string(),
            codec: "codec2".to_string(),
            mode: "mode2".to_string(),
            metadata: "metadata2".to_string(),
        };

        let call_details3 = CallDetails {
            call_leg_id: "leg2".to_string(),
            client_address: "address21".to_string(),
            codec: "codec2".to_string(),
            mode: "mode2".to_string(),
            metadata: "metadata2".to_string(),
        };

        db_client.insert(call_details1);

        assert_eq!(1, db_client.select_all().len());
        db_client.insert(call_details2);
        assert_eq!(2, db_client.select_all().len());
        db_client.insert(call_details3);
        assert_eq!(3, db_client.select_all().len());

        match db_client.select_by_call_id_and_address("leg2".to_string(), "address2".to_string()) {
            Ok(Row) => { assert_eq!("leg2".to_string(), Row.call_leg_id); }
            Err(_) => { assert!(false, "db row should exist"); } // dummy failures
        }

        match db_client.select_by_call_id_and_address("leg3".to_string(), "address3".to_string()) {
            Ok(Row) => { assert!(false, "db row should not exist"); }
            Err(e) => { assert!(true); } // dummy failures
        }

        db_client.delete_by_call_leg_id("leg1".to_string());
        assert_eq!(2, db_client.select_all().len());
        db_client.delete_by_call_leg_and_client_address("leg2".to_string(), "address21".to_string());
        assert_eq!(1, db_client.select_all().len());
    }
}