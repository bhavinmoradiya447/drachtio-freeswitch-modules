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

    pub fn select_by_call_id_and_address(&self, call_leg_id: String, client_address: String) -> Result<CallDetails, Err> {
        let mut stmt =
            self.connection.prepare("SELECT * FROM  from CALL_DETAILS where CALL_LEG_ID= :callId and CLIENT_ADDRESS= :address").unwrap();

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
            Err(e) => { Err(e) }
            _ => {
                info!("Got Other then row");
                Err("Got Other then row")
            }
        }
    }
}