use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use fantoccini::{Client, ClientBuilder};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use tokio::{sync::RwLock, spawn, time::sleep};

#[derive(Debug)]
pub enum Error {
    NoClientsAvailable,
    NoSuchSession,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub client_wrapper: Option<ClientWrapper>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct ClientWrapper {
    // TODO: if there was a way of getting a webdriver from a client
    // We could do without a wrapper
    pub webdriver: String,
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
    pub async fn new(webdrivers: Vec<String>) -> Arc<RwLock<Self>> {
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
                        lock.release_session(id).await.unwrap();
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
            let used_webdrivers: Vec<String> = self.sessions
                .iter()
                .map(|(_, s)| s.client_wrapper.clone())
                .filter(|cw| cw.is_some())
                .map(|cw| cw.unwrap())
                .map(|cw| cw.webdriver)
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

        let client_wrapper = ClientWrapper { 
            webdriver, 
            client,
        };

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
    pub async fn release_session(&mut self, id: String) -> Result<(), Error> {
        let session = self.sessions.remove(&id).ok_or(Error::NoSuchSession)?;

        if let Some(wrapper) = session.client_wrapper.clone() {
            wrapper.client.close().await.unwrap();
            println!("Session {id} client was closed.");
        } else {
            println!("Session {id} was already closed. Skipping");
        }

        Ok(())
    }

    // Destroys all sessions for graceful shutdown
    pub async fn clear(&mut self) -> Result<(), Error> {
        let session_ids: Vec<String> = self.sessions.iter().map(|(id, _)| id.clone()).collect();
        for id in session_ids {
            self.release_session(id).await?;
        }

        Ok(())
    }
}