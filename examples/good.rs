use std::time::Duration;

use fantoccini_session_manager::FantocciniConnectionManager;
use tokio::{spawn, time::sleep};

#[tokio::main]
async fn main() {
    let links = vec!["http://localhost:4444", "http://localhost:4445"]
        .into_iter()
        .map(|link| link.to_string())
        .collect();
    let manager = FantocciniConnectionManager::new(links);

    {
        let manager = manager.clone();
        spawn(async move {
            let (a, b) = {
                let manager = manager.clone();
                let mut lock = manager.write().await;
                let a = lock
                    .create_session(Some(Duration::from_secs(20)))
                    .await
                    .unwrap();
                let b = lock
                    .create_session(Some(Duration::from_secs(10)))
                    .await
                    .unwrap();

                (a, b)
            };

            let a_id = a.id.clone();

            spawn(async move {
                println!("{}: Going to google", a.id);
                a.client.goto("https://google.com").await.unwrap();
            });

            spawn(async move {
                println!("{}: Going to barbora", b.id);
                b.client.goto("https://barbora.lt").await.unwrap();
            });

            spawn(async move {
                let sess = {
                    let lock = manager.read().await;
                    lock.get_session(a_id).await.unwrap()
                };

                let id = &sess.id;
                println!("Got session {id}");
            })
        });
    }

    sleep(Duration::from_secs(10)).await;

    {
        let mut lock = manager.write().await;
        lock.clear().await.unwrap();
    }

    println!("Done");
}
