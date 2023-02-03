Warning: This crate is in active development. Its public signatures may change drastically.

# fantoccini-session-manager
I solved a problem with fantoccini and geckodriver - specificly, the ability to manage multiple connections in one application. 

First of all, what is fantoccini: https://github.com/jonhoo/fantoccini
Fantoccini is an abstraction on A high-level API for programmatically interacting with web pages through WebDriver.

We actually use fantoccini via our tests because our testing library depends on it.

But anyways, The problem is specificly with geckodriver - the websdriver for firefox. You cannot connect to geckodriver if a session has already beed started. In order to be able to connect to a geckodriver again you need to gracefully close the client. This is more often a good thing because it helps with resource management - like ram, cpu usage, etc., but has some its own issues as I'll demonstrate in a sec.

See example "sad". If we run this application once, it will work as expected. (run sad example) If we run it a second time, the application will crash mentioning that a session is already running on the geckodriver instance (run sad example).

The proper way to be able to run code similar to the sad example, is by issuing the close command as in the "mid" example (rin mid example). And now we should be able to run the example again without issue.

This is problematic for when apis crash or forget to gracefully close the client. Meaning that the geckodriver would have to be manually restarted, which is a pain in the ass.

The way I decided to solve/mitigate this issue is by having a piece of code that issues fantoccini sessions which expire after some amount of time. That's where the whole "fantoccini-session-manager" comes in.

The library still has an issue if the application crashes, but as long as we have it running - which it will if we pair it with actix, axum, etc, the clients should be reusable as long as they're issued by the session manager.
