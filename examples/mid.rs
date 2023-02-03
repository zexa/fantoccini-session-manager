use fantoccini::ClientBuilder;

#[tokio::main]
async fn main() {
    let webdriver = "http://localhost:4444";
    let client = ClientBuilder::native()
        .connect(webdriver)
        .await
        .expect("Failed to connect to webdriver");
    client
        .goto("https://google.com")
        .await
        .expect("Failed to open google");
    client.close().await.expect("Failed to close client");
}
