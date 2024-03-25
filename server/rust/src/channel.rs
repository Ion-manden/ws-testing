use std::collections::HashMap;

use crate::connection;
use ractor::{Actor, ActorId, ActorProcessingErr, ActorRef};

pub struct Channel;

/// This is the types of message [PingPong] supports
#[derive(Debug, Clone)]
pub enum Message {
    Join(ActorRef<connection::Message>),
    Leave(ActorRef<connection::Message>),
    Message(String),
}

pub struct ChannelState {
    attendies: HashMap<ActorId, ActorRef<connection::Message>>,
}

// the implementation of our actor's "logic"
impl Actor for Channel {
    // An actor has a message type
    type Msg = Message;
    // and (optionally) internal state
    type State = ChannelState;
    // Startup initialization args
    type Arguments = ();

    // Initially we need to create our state, and potentially
    // start some internal processing (by posting a message for
    // example)
    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        // create the initial state
        Ok(ChannelState {
            attendies: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            Message::Join(conn) => {
                state.attendies.insert(conn.get_id(), conn);
            }
            Message::Leave(conn) => {
                state.attendies.remove(&conn.get_id());
            }
            Message::Message(msg) => {
                for (id, conn) in state.attendies.clone() {
                    match conn.send_message(connection::Message::Out(msg.clone())) {
                        Ok(_) => (),
                        Err(err) => match err {
                            ractor::MessagingErr::SendErr(_)
                            | ractor::MessagingErr::ChannelClosed => {
                                println!("Channel Closed");
                                state.attendies.remove(&id);
                            }
                            ractor::MessagingErr::InvalidActorType => {
                                println!("Invalid actor type")
                            }
                        },
                    }
                }
            }
        }

        Ok(())
    }
}
