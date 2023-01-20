use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use fantoccini::{Client, ClientBuilder};
use tokio::{spawn, sync::RwLock, time::sleep};

#[derive(Debug)]
enum Error {
    NoClientsAvailable,
}

#[derive(Clone, Debug)]
struct Session {
    // The client will be None once the session expires
    client: Option<Client>,

    expires_at: Option<DateTime<Utc>>,
}

impl Session {
    pub fn has_expired(&self) -> bool {
        if self.expires_at.is_none() {
            return false;
        }

        Utc::now() > self.expires_at.unwrap()
    }
}

struct FantocciniConnectionManager {
    unallocated_clients: Vec<Client>,
    sessions: HashMap<String, Arc<Session>>,
}

impl FantocciniConnectionManager {
    pub async fn new(urls: Vec<impl Into<String>>) -> Arc<RwLock<Self>> {
        let mut unallocated_clients = vec![];
        let builder = ClientBuilder::native();
        // TODO: Should either close previous connections or skip connection on failure.
        for webdriver in urls {
            let client = builder.clone().connect(&webdriver.into()).await.unwrap();
            // let client = Arc::new(RwLock::new(client));

            unallocated_clients.push(client);
        }

        let sessions = HashMap::<String, Arc<Session>>::new();
        let this = Self {
            unallocated_clients,
            sessions,
        };
        let this = Arc::new(RwLock::new(this));

        // Release clients from expired sessions
        {
            let this = this.clone();
            spawn(async move {
                loop {
                    sleep(Duration::from_secs(1)).await;
                    println!("Checking");
                    let mut lock = this.write().await;

                    let sessions_active = lock.sessions.keys().count();
                    println!("Got lock. Sessions active: {sessions_active}");

                    let mut sessions = HashMap::<String, Arc<Session>>::new();
                    for (id, session) in lock.sessions.clone().drain() {
                        println!("Checking session {id}");
                        if session.has_expired() {
                            println!("Session {id} has expired. Closing client.");

                            if let Some(client) = session.client.clone() {
                                client.close().await.unwrap();

                                // TODO: Re-add client to unallocated_clients
                                // will likely need to establish the connection again
                            }

                            continue;
                        }

                        sessions.insert(id, session);
                    }

                    lock.sessions = sessions;
                }
            });
        };

        this
    }

    // Creates a session for the given duration
    pub async fn create_session(
        &mut self,
        duration: Option<Duration>,
    ) -> Result<Arc<Session>, Error> {
        let client = self
            .unallocated_clients
            .pop()
            .ok_or(Error::NoClientsAvailable)?;
        let id = client.session_id().await.unwrap().unwrap();

        let expires_at = match duration {
            None => None,
            Some(duration) => Some(Utc::now() + chrono::Duration::from_std(duration).unwrap()),
        };

        let session = Session {
            client: Some(client),
            expires_at,
        };
        let session = Arc::new(session);

        self.sessions.insert(id, session.clone());

        Ok(session)
    }
}

#[tokio::main]
async fn main() {
    let links = vec!["http://localhost:4444", "http://localhost:4445"];
    let manager = FantocciniConnectionManager::new(links).await;

    {
        let manager = manager.clone();
        spawn(async move {
            {
                let mut lock = manager.write().await;
                let a = lock
                    .create_session(Some(Duration::from_secs(5)))
                    .await
                    .unwrap();
                let b = lock
                    .create_session(Some(Duration::from_secs(10)))
                    .await
                    .unwrap();
            }

            sleep(Duration::from_secs(10)).await;
        });
    }

    spawn(async move {
        sleep(Duration::from_secs(15)).await;

        let mut lock = manager.write().await;
        let a = lock
            .create_session(Some(Duration::from_secs(5)))
            .await
            .unwrap();
        let b = lock
            .create_session(Some(Duration::from_secs(10)))
            .await
            .unwrap();
    });

    sleep(Duration::from_secs(50)).await;

    println!("Done");
}
