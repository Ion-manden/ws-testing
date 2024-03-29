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

use std::net::SocketAddr;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

//allows to split the websocket stream into separate TX and RX branches
use futures::stream::StreamExt;

pub async fn run() {
    let channel_handler = channel::ChannelActorHandle::new();

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
        .with_state(channel_handler)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

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
    State(channel_handler): State<channel::ChannelActorHandle>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, channel_handler))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(
    socket: WebSocket,
    who: SocketAddr,
    channel_handler: channel::ChannelActorHandle,
) {
    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    let (sender, mut receiver) = socket.split();
    let connection_state = connection::ConnectionState {
        ws: sender,
        channel_actor: channel_handler,
    };

    let connection_handler = connection::ConnectionActorHandle::new(connection_state);

    // This second task will receive messages from client and print them on server console
    let conn_handler_ref = connection_handler.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(msg) => {
                    conn_handler_ref
                        .send_message(connection::ActorMessage::In(msg))
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
    connection_handler.send_message(connection::ActorMessage::Close).unwrap();
}
