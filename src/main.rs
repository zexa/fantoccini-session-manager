use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use fantoccini::{Client, ClientBuilder};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use tokio::{spawn, sync::RwLock, time::sleep};

#[derive(Debug)]
enum Error {
    NoClientsAvailable,
    NoSuchSession,
    SessionExpired,
}

#[derive(Clone, Debug)]
struct Session {
    id: String,
    // The client will be None once the session expires
    client_wrapper: Option<ClientWrapper>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
struct ClientWrapper {
    webdriver: String,
    client: Client,
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
    builder: ClientBuilder<HttpsConnector<HttpConnector>>,
    unallocated_clients: Vec<ClientWrapper>,
    sessions: HashMap<String, Arc<Session>>,
}

impl FantocciniConnectionManager {
    pub async fn new(urls: Vec<impl ToString>) -> Arc<RwLock<Self>> {
        let mut unallocated_clients = vec![];
        let builder = ClientBuilder::native();
        // TODO: Should either close previous connections or skip connection on failure.
        for webdriver in urls {
            let webdriver = webdriver.to_string();
            let client = builder.clone().connect(&webdriver).await.unwrap();
            let wrapper = ClientWrapper {
                webdriver,
                client
            };
            
            unallocated_clients.push(wrapper);
        }

        let sessions = HashMap::<String, Arc<Session>>::new();
        let this = Self {
            builder: builder.clone(),
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
                    let lock = this.try_write();
                    if let Err(_) = lock {
                        println!("FantocciniConnectionManager bussy. Will try again later");

                        continue;
                    }
                    let mut lock = lock.unwrap();

                    let sessions_active = lock.sessions.keys().count();
                    println!("Got lock. Sessions active: {sessions_active}");

                    let mut expired_session_ids = vec![];
                    for (id, session) in lock.sessions.iter() {
                        println!("Checking session {id}");
                        if session.has_expired() {
                            println!("Session {id} has expired. Marking.");
                            expired_session_ids.push(id.clone());
                        } else {
                            println!("Session {id} has not expired. Skipping.");
                        }
                    }

                    for id in expired_session_ids {
                        println!("Session {id} has expired. Releasing.");
                        lock.release_session(id, true).await.unwrap();
                    }
                }
            });
        };

        this
    }

    // Creates a session for the given duration (if any)
    // Once the session expires the underlying client in session.client will become None
    pub async fn create_session(
        &mut self,
        duration: Option<Duration>,
    ) -> Result<Arc<Session>, Error> {
        let client_wrapper = self
            .unallocated_clients
            .pop()
            .ok_or(Error::NoClientsAvailable)?;
        let id = client_wrapper.client.session_id().await.unwrap().unwrap();

        let expires_at = duration
            .map(|duration| Utc::now() + chrono::Duration::from_std(duration).unwrap());

        let session = Session {
            id: id.clone(),
            client_wrapper: Some(client_wrapper),
            expires_at,
        };
        let session = Arc::new(session);

        self.sessions.insert(id, session.clone());

        Ok(session)
    }

    // Releases a session by putting it back into unallocated clients
    pub async fn release_session(&mut self, id: String, add_back_to_pool: bool) -> Result<(), Error> {
        let session = self.sessions.remove(&id).ok_or(Error::NoSuchSession)?;

        if let Some(wrapper) = session.client_wrapper.clone() {
            wrapper.client.close().await.unwrap();
            println!("Session {id} client was closed. Building new connection.");

            let client = self.builder.connect(&wrapper.webdriver).await.unwrap();
            println!("Session {id} connection was built. Adding to unallocated_clients.");
            let webdriver = wrapper.webdriver;
            let wrapper = ClientWrapper {
                client,
                webdriver,
            };

            if add_back_to_pool {
                self.unallocated_clients.push(wrapper);
            }
            println!("Session {id} added to unallocated_clients. Yay.");
        } else {
            println!("Session {id} was already closed. Skipping");
        }

        Ok(())
    }

    // Destroys all sessions for graceful shutdown
    pub async fn clear(&mut self) -> Result<(), Error> {
        let session_ids: Vec<String> = self.sessions.iter().map(|(id, _)| id.clone()).collect();
        for id in session_ids {
            self.release_session(id, false).await?;
        }

        loop {
            let cw = self.unallocated_clients.pop();

            if let None = cw {
                break;
            }
            let cw = cw.unwrap();
            let client = cw.client;
            if let Err(e) = client.close().await {
                eprintln!("{e:?}");
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let links = vec!["http://localhost:4444", "http://localhost:4445"];
    let manager = FantocciniConnectionManager::new(links).await;

    {
        let manager = manager.clone();
        spawn(async move {
            let (a, b) = {
                let mut lock = manager.write().await;
                let a = lock
                    .create_session(Some(Duration::from_secs(5)))
                    .await
                    .unwrap();
                let b = lock
                    .create_session(Some(Duration::from_secs(10)))
                    .await
                    .unwrap();

                (a, b)
            };
        });
    }

    sleep(Duration::from_secs(20)).await;

    println!("Done");
}
