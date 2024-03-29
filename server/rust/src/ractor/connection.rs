use axum::extract::ws::{self, WebSocket};
use futures::SinkExt;
use futures_util::stream::SplitSink;
use ractor::{Actor, ActorProcessingErr, ActorRef};

use super::balancer;

pub struct Connection;

/// This is the types of message [PingPong] supports
#[derive(Debug, Clone)]
pub enum Message {
    In(String),
    Out(String),
    Close,
}

pub struct ConnectionState {
    pub ws: SplitSink<WebSocket, ws::Message>,
    pub balancer_actor: ActorRef<balancer::Message>,
}

// the implementation of our actor's "logic"
impl Actor for Connection {
    // An actor has a message type
    type Msg = Message;
    // and (optionally) internal state
    type State = ConnectionState;
    // Startup initialization args
    type Arguments = ConnectionState;

    // Initially we need to create our state, and potentially
    // start some internal processing (by posting a message for
    // example)
    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        state: ConnectionState,
    ) -> Result<Self::State, ActorProcessingErr> {
        state
            .balancer_actor
            .send_message(balancer::Message::Join(balancer::DownsteamActor::Connection(myself)))
            .unwrap();

        Ok(state)
    }

    // This is our main message handler
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            Message::In(msg) => {
                state
                    .balancer_actor
                    .send_message(balancer::Message::In(msg))
                    .unwrap();
            }
            Message::Out(msg) => {
                state.ws.send(ws::Message::Text(msg)).await.unwrap();
            }
            Message::Close => {
                state
                    .balancer_actor
                    .send_message(balancer::Message::Leave(balancer::DownsteamActor::Connection(myself)))
                    .unwrap();
            }
        };

        Ok(())
    }
}
