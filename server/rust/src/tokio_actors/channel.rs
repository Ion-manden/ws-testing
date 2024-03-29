use std::collections::HashMap;

use super::connection;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ChannelActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl ChannelActorHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(500);
        let actor = ChannelActor::new(receiver);
        tokio::spawn(run(actor));

        Self { sender }
    }

    pub fn send_message(
        &self,
        msg: ActorMessage,
    ) -> Result<(), mpsc::error::TrySendError<ActorMessage>> {
        self.sender.try_send(msg)
    }
}

pub struct ChannelState {
    attendies: HashMap<i32, connection::ConnectionActorHandle>,
}

pub struct ChannelActor {
    receiver: mpsc::Receiver<ActorMessage>,
    state: ChannelState,
}

pub enum ActorMessage {
    Join(connection::ConnectionActorHandle),
    Leave(connection::ConnectionActorHandle),
    Message(String),
}

impl ChannelActor {
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self {
            receiver,
            state: ChannelState {
                attendies: HashMap::new(),
            },
        }
    }
    fn handle_message(&mut self, msg: ActorMessage) {
        match msg {
            ActorMessage::Join(conn) => {
                self.state.attendies.insert(conn.get_id(), conn);
            }
            ActorMessage::Leave(conn) => {
                self.state.attendies.remove(&conn.get_id());
            }
            ActorMessage::Message(msg) => {
                for (_id, conn) in self.state.attendies.clone() {
                    conn.send_message(connection::ActorMessage::Out(msg.clone()))
                        .unwrap();
                }
            }
        }
    }
}

async fn run(mut actor: ChannelActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
