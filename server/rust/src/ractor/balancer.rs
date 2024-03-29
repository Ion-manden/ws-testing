use std::collections::HashMap;

use super::channel;
use super::connection;
use ractor::{Actor, ActorId, ActorProcessingErr, ActorRef};

pub struct Balancer;

#[derive(Debug, Clone)]
pub enum DownsteamActor {
    Balancer(ActorRef<Message>),
    Connection(ActorRef<connection::Message>),
}

/// This is the types of message [Balancer] supports
#[derive(Debug, Clone)]
pub enum Message {
    Join(DownsteamActor),
    Leave(DownsteamActor),
    In(String),
    Out(String),
}

pub enum UpstreamActor {
    Balancer(ActorRef<Message>),
    Channel(ActorRef<channel::Message>),
}

pub struct BalancerState {
    attendies: HashMap<ActorId, DownsteamActor>,
    upstream: UpstreamActor,
}

// the implementation of our actor's "logic"
impl Actor for Balancer {
    // An actor has a message type
    type Msg = Message;
    // and (optionally) internal state
    type State = BalancerState;
    // Startup initialization args
    type Arguments = UpstreamActor;

    // Initially we need to create our state, and potentially
    // start some internal processing (by posting a message for
    // example)
    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        upstream: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        match upstream {
            UpstreamActor::Balancer(ref actor) => {
                actor
                    .send_message(Message::Join(DownsteamActor::Balancer(myself)))
                    .unwrap();
            }
            UpstreamActor::Channel(ref actor) => {
                actor.send_message(channel::Message::Join(myself)).unwrap();
            }
        }

        // create the initial state
        Ok(BalancerState {
            attendies: HashMap::new(),
            upstream,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            Message::Join(conn) => match conn {
                DownsteamActor::Balancer(conn) => {
                    state
                        .attendies
                        .insert(conn.get_id(), DownsteamActor::Balancer(conn));
                }
                DownsteamActor::Connection(conn) => {
                    state
                        .attendies
                        .insert(conn.get_id(), DownsteamActor::Connection(conn));
                }
            },
            Message::Leave(conn) => match conn {
                DownsteamActor::Balancer(conn) => {
                    state.attendies.remove(&conn.get_id());
                }
                DownsteamActor::Connection(conn) => {
                    state.attendies.remove(&conn.get_id());
                }
            },
            Message::In(msg) => match &state.upstream {
                UpstreamActor::Balancer(actor) => {
                    actor.send_message(Message::In(msg)).unwrap();
                }
                UpstreamActor::Channel(actor) => {
                    actor.send_message(channel::Message::Message(msg)).unwrap();
                }
            },
            Message::Out(msg) => {
                for (id, conn) in state.attendies.clone() {
                    match conn {
                        DownsteamActor::Balancer(conn) => {
                            match conn.send_message(Message::Out(msg.clone())) {
                                Ok(_) => (),
                                Err(err) => match err {
                                    ractor::MessagingErr::SendErr(_)
                                    | ractor::MessagingErr::ChannelClosed => {
                                        println!("Balancer Closed");
                                        state.attendies.remove(&id);
                                    }
                                    ractor::MessagingErr::InvalidActorType => {
                                        println!("Invalid actor type")
                                    }
                                },
                            }
                        }
                        DownsteamActor::Connection(conn) => {
                            match conn.send_message(connection::Message::Out(msg.clone())) {
                                Ok(_) => (),
                                Err(err) => match err {
                                    ractor::MessagingErr::SendErr(_)
                                    | ractor::MessagingErr::ChannelClosed => {
                                        println!("Balancer Closed");
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
            }
        }

        Ok(())
    }
}
