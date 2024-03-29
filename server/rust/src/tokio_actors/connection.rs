use super::channel;
use axum::extract::ws::{self, WebSocket};
use futures::SinkExt;
use futures_util::stream::SplitSink;
use rand::Rng;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ConnectionActorHandle {
    id: i32,
    sender: mpsc::Sender<ActorMessage>,
}

impl ConnectionActorHandle {
    pub fn new(state: ConnectionState) -> Self {
        let mut rng = rand::thread_rng();
        let (sender, receiver) = mpsc::channel(500);

        let handler = Self {
            id: rng.gen(),
            sender,
        };

        state
            .channel_actor
            .send_message(channel::ActorMessage::Join(handler.clone()))
            .unwrap();

        let actor = ConnectionActor::new(receiver, state, handler.clone());
        tokio::spawn(run(actor));

        handler
    }
    pub fn get_id(self: &Self) -> i32 {
        self.id
    }

    pub fn send_message(
        &self,
        msg: ActorMessage,
    ) -> Result<(), mpsc::error::TrySendError<ActorMessage>> {
        self.sender.try_send(msg)
    }
}

pub struct ConnectionState {
    pub ws: SplitSink<WebSocket, ws::Message>,
    pub channel_actor: channel::ChannelActorHandle,
}

struct ConnectionActor {
    receiver: mpsc::Receiver<ActorMessage>,
    state: ConnectionState,
    handler: ConnectionActorHandle,
}

#[derive(Debug)]
pub enum ActorMessage {
    In(String),
    Out(String),
    Close,
}

impl ConnectionActor {
    fn new(
        receiver: mpsc::Receiver<ActorMessage>,
        state: ConnectionState,
        handler: ConnectionActorHandle,
    ) -> Self {
        let actor = Self {
            receiver,
            state,
            handler,
        };

        actor
    }
    async fn handle_message(&mut self, msg: ActorMessage) {
        let channel_actor = self.state.channel_actor.clone();
        match msg {
            ActorMessage::In(msg) => {
                channel_actor
                    .send_message(channel::ActorMessage::Message(msg))
                    .unwrap();
            }
            ActorMessage::Out(msg) => {
                self.state.ws.send(ws::Message::Text(msg)).await.unwrap();
            }
            ActorMessage::Close => {
                channel_actor
                    .send_message(channel::ActorMessage::Leave(self.handler.clone()))
                    .unwrap();
            }
        };
    }
}

async fn run(mut actor: ConnectionActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}
