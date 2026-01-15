#[macro_use]
extern crate rocket;

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use rocket::tokio::sync::{RwLock, mpsc::{unbounded_channel, UnboundedSender}};

struct Shared {
    clients: RwLock<Vec<(usize, UnboundedSender<ws::Message>)>>,
    counter: AtomicUsize,
}

type SharedState = Arc<Shared>;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/echo")]
fn echo_channel(ws: ws::WebSocket, state: &rocket::State<SharedState>) -> ws::Channel<'static> {
    use rocket::futures::{StreamExt, SinkExt};

    let state = state.inner().clone();

    ws.channel(move |mut stream| Box::pin(async move {
        let my_id = state.counter.fetch_add(1, Ordering::Relaxed);

        let (tx, mut rx) = unbounded_channel::<ws::Message>();

        // register this client's sender
        {
            let mut clients = state.clients.write().await;
            clients.push((my_id, tx));
        }

        // main loop: concurrently read incoming websocket messages and
        // outgoing messages sent to this client's channel
        loop {
            rocket::tokio::select! {
                incoming = stream.next() => {
                    match incoming {
                        Some(Ok(msg)) => {
                            println!("Received: {:?}", msg);

                            // broadcast to all other clients
                            let clients = state.clients.read().await;
                            for (id, sender) in clients.iter() {
                                if *id != my_id {
                                    let _ = sender.send(msg.clone());
                                }
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("WebSocket error: {:?}", e);
                            break;
                        }
                        None => break,
                    }
                }
                Some(out) = rx.recv() => {
                    let _ = stream.send(out).await;
                }
            }
        }

        // connection is closing: remove from registry
        {
            let mut clients = state.clients.write().await;
            clients.retain(|(id, _)| *id != my_id);
        }

        Ok(())
    }))
}

#[launch]
fn rocket() -> _{
    let shared = Arc::new(Shared {
        clients: RwLock::new(Vec::new()),
        counter: AtomicUsize::new(1),
    });

    rocket::build()
        .manage(shared)
        .mount("/", routes![index, echo_channel])
}
