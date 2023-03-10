use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use fantoccini::{Client, ClientBuilder};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use tokio::{spawn, sync::RwLock, time::sleep};

#[derive(Debug)]
pub enum Error {
    NoClientsAvailable,
    NoSuchSession,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub webdriver: String,

    // TODO: The client structure will remain even if the session is closed, however, any command will fail.
    // It is up for the user to indentify that the session has expired.
    pub client: Client,
}

impl Session {
    pub fn has_expired(&self) -> bool {
        if self.expires_at.is_none() {
            return false;
        }

        Utc::now() > self.expires_at.unwrap()
    }
}

pub struct FantocciniConnectionManager {
    builder: ClientBuilder<HttpsConnector<HttpConnector>>,
    webdrivers: Vec<String>,
    sessions: HashMap<String, Arc<Session>>,
}

impl FantocciniConnectionManager {
    pub fn new(webdrivers: Vec<String>) -> Arc<RwLock<Self>> {
        let builder = ClientBuilder::native();
        let sessions = HashMap::<String, Arc<Session>>::new();
        let this = Self {
            builder: builder.clone(),
            webdrivers,
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
                        lock.release_session(id).await;
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
        // get unused webdriver
        let mut unused_webdrivers: Vec<String> = {
            let used_webdrivers: Vec<String> = self
                .sessions
                .iter()
                .map(|(_, session)| session.webdriver.clone())
                .collect();

            println!("used: {used_webdrivers:?}");

            self.webdrivers
                .iter()
                .filter(|webdriver| !used_webdrivers.contains(webdriver))
                .map(|webdriver| webdriver.to_string())
                .collect()
        };
        println!("unused: {unused_webdrivers:?}");

        let webdriver = unused_webdrivers.pop().ok_or(Error::NoClientsAvailable)?;
        let client = self.builder.connect(&webdriver).await.unwrap();
        let id = client.session_id().await.unwrap().unwrap();

        let expires_at =
            duration.map(|duration| Utc::now() + chrono::Duration::from_std(duration).unwrap());

        let session = Session {
            id: id.clone(),
            webdriver,
            client,
            expires_at,
        };
        let session = Arc::new(session);

        self.sessions.insert(id, session.clone());

        Ok(session)
    }

    pub async fn get_session(&self, id: String) -> Result<Arc<Session>, Error> {
        let session = self.sessions.get(&id).ok_or(Error::NoSuchSession)?;

        Ok(session.clone())
    }

    pub async fn release_session(&mut self, id: String) {
        let session = self.sessions.remove(&id);

        if session.is_none() {
            return;
        }

        let _ = session.unwrap().client.clone().close().await;
    }

    // Destroys all sessions for graceful shutdown
    pub async fn clear(&mut self) -> Result<(), Error> {
        let session_ids: Vec<String> = self.sessions.iter().map(|(id, _)| id.clone()).collect();
        for id in session_ids {
            self.release_session(id).await;
        }

        Ok(())
    }
}
