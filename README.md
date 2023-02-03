Warning: This crate is in active development. Its public signatures may change drastically.

# fantoccini-session-manager
I solved a problem with fantoccini and geckodriver - specificly, the ability to manage multiple connections in one application. 

First of all, what is fantoccini: https://github.com/jonhoo/fantoccini
Fantoccini is an abstraction on A high-level API for programmatically interacting with web pages through WebDriver.

The problem with fantoccini and geckodriver is that you cannot connect to geckodriver if a session has already beed started. In order to be able to connect to a geckodriver again you need to gracefully close the client. This is more often a good thing because it helps with resource management - like ram, cpu usage, etc., but has some its own issues as I'll demonstrate in a sec.

See example "sad". If we run this example once on a fresh geckodriver instance, it will work as expected. If we run it a second time, the application will crash mentioning that a session is already running on the geckodriver instance.

The proper way to be able to run code similar to the "sad" example, is by issuing the close command. See "mid" example. By running this example the first time on a fresh geckodriver instance, the client will gracefully exit, which will allow creating a new session on that geckodriver instance once the mid example is ran a second time.

While gracefully exiting clients is the desired, this approach is problematic due to APIs crashing or developers forgetting to gracefully close the client. This means that the geckodriver would have to be manually restarted, which is a pain in the ass if you wanted to use it a second time.

The way I decided to solve/mitigate this issue is by using a wrapper on fantoccini that issues sessions which expire after some amount of time. The code tracks if a session has expired, gracefully exits it, and also allows you to somewhat comfortably manage multiple fantoccini clients/geckodriver-sesisons.

The library still has an issue if the application crashes, but as long as we have it running - which it will if we pair it with actix, axum, etc, the clients should be reusable as long as they're issued by the session manager.
