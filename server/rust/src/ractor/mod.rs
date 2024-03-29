mod balancer;
mod channel;
mod connection;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_extra::TypedHeader;
use ractor::{Actor, ActorRef};
use rand::Rng;

use std::net::SocketAddr;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

//allows to split the websocket stream into separate TX and RX branches
use futures::stream::StreamExt;

fn get_random_balancer(
    balancers: Vec<ActorRef<balancer::Message>>,
) -> Option<ActorRef<balancer::Message>> {
    if balancers.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();
    let random_index = rng.gen_range(0..balancers.len());

    Some(balancers[random_index].clone())
}

pub async fn run() {
    let (channel_actor, _handle) = Actor::spawn(None, channel::Channel, ())
        .await
        .expect("Failed to start channel actor");

    let layer_1_balancer_count = 5;
    let layer_2_balancer_count = 50;
    let mut layer_2_balancer_actors: Vec<ActorRef<balancer::Message>> = Vec::new();
    for _ in 0..layer_1_balancer_count {
        let (layer_1_balancer_actor, _handle) = Actor::spawn(
            None,
            balancer::Balancer,
            balancer::UpstreamActor::Channel(channel_actor.clone()),
        )
        .await
        .expect("Failed to start layer 1 balancer actor");
        for _ in 0..layer_2_balancer_count {
            let (layer_2_balancer_actor, _handle) = Actor::spawn(
                None,
                balancer::Balancer,
                balancer::UpstreamActor::Balancer(layer_1_balancer_actor.clone()),
            )
            .await
            .expect("Failed to start layer 2 balancer actor");

            layer_2_balancer_actors.push(layer_2_balancer_actor)
        }
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build our application with some routes
    let app = Router::new()
        .route("/global", get(ws_handler))
        // logging so we can see whats going on
        .with_state(layer_2_balancer_actors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // handle
    //     .await
    //     .expect("Ping-pong actor failed to exit properly");

    // run it with hyper
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8888")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

/// The handler for the HTTP request (this gets called when the HTTP GET lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(balancer_actors): State<Vec<ActorRef<balancer::Message>>>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    let balancer_actor = get_random_balancer(balancer_actors).unwrap();
    ws.on_upgrade(move |socket| handle_socket(socket, addr, balancer_actor))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(
    socket: WebSocket,
    who: SocketAddr,
    balancer_actor: ActorRef<balancer::Message>,
) {
    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    let (sender, mut receiver) = socket.split();

    let (conn_actor, _handle) = Actor::spawn(
        None,
        connection::Connection,
        connection::ConnectionState {
            ws: sender,
            balancer_actor: balancer_actor.to_owned(),
        },
    )
    .await
    .unwrap();

    // This second task will receive messages from client and print them on server console
    let conn_actor_ref = conn_actor.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(msg) => {
                    conn_actor_ref
                        .send_message(connection::Message::In(msg))
                        .unwrap();
                }
                _ => (),
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_b = (&mut recv_task) => {
            match rv_b {
                Ok(_) => println!("Connection done"),
                Err(b) => println!("Error receiving messages {b:?}")
            }
        }
    }

    // returning from the handler closes the websocket connection
    println!("Websocket context {who} destroyed");
    conn_actor.send_message(connection::Message::Close).unwrap();
}
