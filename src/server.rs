use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use tokio::net::{TcpListener, TcpStream};
use tungstenite::protocol::Message;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

use crate::game;
use crate::ServerOptions;

async fn handle_connection(
    peer_map: PeerMap,
    game_hosts: game::GameHosts,
    game_lobby: game::GameLobby,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    // Check for game with room code, announce when joining, or close the connection if no room exists
    // XXX: How will I remove the room when the host disconnects?

    let (outgoing, incoming) = ws_stream.split();

    // Process the messages from the clients
    let broadcast_incoming = incoming.try_for_each(|msg| {
        match serde_json::from_str(msg.to_text().unwrap()).unwrap() {
            game::GameMessageType::NewPlayer(mut game_player) => {
                //let mut game_player =
                //serde_json::from_str::<game::GamePlayer>(msg.to_text().unwrap()).unwrap();

                println!("Received a message from {}: {:?}", addr, game_player,);
                let peers = peer_map.lock().unwrap();

                // This should only get run when a host initially connects
                if game_player.room_code.is_none() {
                    let room_code = game::generate_room_code();
                    game_player.room_code = Some(room_code.clone());
                    game_player.peer_addr = Some(addr.clone());

                    // We want to broadcast the message to ONLY ourselves.
                    let broadcast_recipients = peers
                        .iter()
                        .filter(|(peer_addr, _)| peer_addr == &&addr)
                        .map(|(_, ws_sink)| ws_sink);

                    for recp in broadcast_recipients {
                        println!("Send a message to the game host");
                        recp.unbounded_send(Message::binary(
                            serde_json::to_string(&game::GameMessageType::NewPlayer(
                                game_player.clone(),
                            ))
                            .unwrap(),
                        ))
                        .unwrap();
                    }

                    // Add room code and host to the game lobby
                    game_hosts.lock().unwrap().insert(addr, room_code.clone());
                    game_lobby
                        .lock()
                        .unwrap()
                        .insert(room_code, vec![game_player.clone()]);
                };

                // Hack for when any other client (non-host) initially connects
                if game_player.peer_addr.is_none() {
                    game_player.peer_addr = Some(addr.clone());

                    // We want to broadcast the message to ONLY ourselves.
                    let broadcast_recipients = peers
                        .iter()
                        .filter(|(peer_addr, _)| peer_addr == &&addr)
                        .map(|(_, ws_sink)| ws_sink);

                    for recp in broadcast_recipients.clone() {
                        println!("Send a message to the game player");
                        recp.unbounded_send(Message::binary(
                            serde_json::to_string(&game::GameMessageType::NewPlayer(
                                game_player.clone(),
                            ))
                            .unwrap(),
                        ))
                        .unwrap();
                    }

                    // Add room code and host to the game lobby
                    let room_code = game_player.room_code.clone().unwrap();

                    let mut lobby_members = game_lobby
                        .lock()
                        .unwrap()
                        .get_mut(&room_code.clone())
                        .unwrap()
                        .clone();

                    // Tell the connecting player who is in the game lobby
                    for recp in broadcast_recipients {
                        println!("Send a message to the game player");
                        recp.unbounded_send(Message::binary(
                            serde_json::to_string(&game::GameMessageType::AllPlayers(
                                lobby_members.clone(),
                            ))
                            .unwrap(),
                        ))
                        .unwrap();
                    }

                    lobby_members.push(game_player.clone());

                    game_lobby
                        .lock()
                        .unwrap()
                        .insert(room_code, lobby_members.to_owned());
                };

                // We want to broadcast the message to everyone except ourselves.
                let broadcast_recipients = peers
                    .iter()
                    .filter(|(peer_addr, _)| peer_addr != &&addr)
                    .map(|(_, ws_sink)| ws_sink);

                for recp in broadcast_recipients {
                    println!("Send a message to everyone, but the sender");
                    recp.unbounded_send(Message::binary(
                        serde_json::to_string(&game::GameMessageType::NewPlayer(
                            game_player.clone(),
                        ))
                        .unwrap(),
                    ))
                    .unwrap();
                }
            }
            game::GameMessageType::AllPlayers(players) => println!("all player"),
        }

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);

    // Remove any game lobby and collect
    // Check if addr is hosting any games
    let mut peers_to_disconnect = vec![addr];

    let mut hosts = game_hosts.lock().unwrap();

    if let Some(room_code) = hosts.get(&addr) {
        println!("Host is disconnecting");
        // TODO: Collect players in game lobby for disconnection
        // If true, then remove game room from game lobby
        let all_lobby_members = game_lobby.lock().unwrap().remove(room_code).unwrap();

        let lobby_peers = all_lobby_members
            .iter()
            .filter(|player| player.peer_addr.clone().unwrap() != addr)
            .map(|p| p);

        for peers in lobby_peers {
            peers_to_disconnect.push(peers.peer_addr.clone().unwrap());
        }

        drop(game_lobby);
    }

    // then remove host from game hosts
    hosts.remove(&addr);

    drop(hosts);

    // TODO: Remove connections for any players in game rooms that have been closed
    for peer in peers_to_disconnect {
        peer_map.lock().unwrap().remove(&peer);
    }
}

pub async fn start_server(opts: ServerOptions) {
    let addr = opts.server_listen_addr;

    let peer_state = PeerMap::new(Mutex::new(HashMap::new()));
    let game_lobby = game::GameLobby::new(Mutex::new(HashMap::new()));
    let game_hosts = game::GameHosts::new(Mutex::new(HashMap::new()));

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let mut listener = try_socket.expect("Failed to bind");
    println!("Listening on: {}", addr);

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(
            peer_state.clone(),
            game_hosts.clone(),
            game_lobby.clone(),
            stream,
            addr,
        ));
    }
}
