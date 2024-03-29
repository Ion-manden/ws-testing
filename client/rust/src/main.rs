use core::time;
use std::thread;

use tungstenite::{connect, Message};
use url::Url;

fn main() {
    let worker_count: i32 = 1000;

    let mut handlers = Vec::new();
    for id in 0..worker_count {
        let join_handel = thread::spawn(move || start_client(id, worker_count));

        handlers.push(join_handel);
    }

    thread::sleep(time::Duration::from_secs(1));

    let (mut socket, _response) =
        connect(Url::parse("ws://localhost:8888/global").unwrap()).expect("Can't connect");

    socket.send(Message::Text("1".into())).unwrap();

    for handel in handlers {
        handel.join().unwrap();
    }
}

fn start_client(id: i32, worker_count: i32) {
    let (mut socket, _response) =
        connect(Url::parse("ws://localhost:8888/global").unwrap()).expect("Can't connect");

    loop {
        let msg = socket.read().expect("Error reading message");
        println!("ID: {}, Received: {}", id, msg);
        match msg {
            Message::Text(msg) => {
                let nr: i32 = msg.parse().unwrap();
                if nr % worker_count == id {
                    socket.send(Message::Text(format!("{}", nr + 1))).unwrap();
                }
            }
            Message::Binary(_) => todo!(),
            Message::Ping(_) => todo!(),
            Message::Pong(_) => todo!(),
            Message::Close(_) => todo!(),
            Message::Frame(_) => todo!(),
        }
    }
    // socket.close(None);
}
