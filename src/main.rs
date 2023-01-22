use std::time::Duration;

use fantoccini_session_pool::FantocciniConnectionManager;
use tokio::{spawn, time::sleep};



#[tokio::main]
async fn main() {
    let links = vec!["http://localhost:4444", "http://localhost:4445"]
        .into_iter()
        .map(|link| link.to_string())
        .collect();
    let manager = FantocciniConnectionManager::new(links).await;

    {
        let manager = manager.clone();
        spawn(async move {
            let (a, b) = {
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

            {
                println!("{}: Going to google", a.id);
                a.client_wrapper.as_ref().unwrap().client.goto("https://google.com").await.unwrap();
            }

            {
                println!("{}: Going to barbora", b.id);
                b.client_wrapper.as_ref().unwrap().client.goto("https://barbora.lt").await.unwrap();
            }
        });
    }

    sleep(Duration::from_secs(10)).await;

    {
        let mut lock = manager.write().await;
        lock.clear().await.unwrap();
    }

    println!("Done");
}
