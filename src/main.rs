// #![deny(warnings)]
use std::collections::HashMap;
use std::iter;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures::{FutureExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use warp::ws::{Message, WebSocket};
use warp::Filter;

use serde::{Deserialize, Serialize};
use serde_json::json;

use log::info;

use rand::{
    distributions::{Alphanumeric, Distribution, Standard},
    thread_rng, Rng,
};

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

type Games = Arc<RwLock<HashMap<String, GameLobbyState>>>;

#[derive(Deserialize, Serialize, Debug)]
enum GameLobbyResponse {
    PartyUpdate {
        room_code: String,
        users: Vec<String>,
    },
    GameStart {
        room_code: String,
    },
    ServerHand {
        hand: RPSHand,
    },
}

#[derive(Deserialize, Serialize, Debug)]
enum GameLobbyRequest {
    UserLogin {
        user_name: String,
        user_type: UserType,
        room_code: String,
    },
    HostNewGame {
        user_name: String,
        user_type: UserType,
    },
    HostStartGame {
        room_code: String,
    },
    PlayerHand {
        user_name: String,
        room_code: String,
        hand: RPSHand,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
enum RPSHand {
    Rock,
    Paper,
    Scissors,
}

impl Distribution<RPSHand> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> RPSHand {
        match rng.gen_range(0, 3) {
            0 => RPSHand::Rock,
            1 => RPSHand::Paper,
            _ => RPSHand::Scissors,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
enum UserType {
    Host,
    Player,
}

#[derive(Debug, Clone, Default)]
struct GameLobbyState {
    game_started: bool,
    users: Vec<UserServerSideState>,
}

#[derive(Debug, Clone)]
struct UserServerSideState {
    user_id: usize,
    user_name: String,
    user_type: UserType,
    connected: bool,
    channel: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>,
    // score: usize,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Users::default();
    let games = Games::default();
    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

    let games = warp::any().map(move || games.clone());

    // GET /ws -> websocket upgrade
    let ws = warp::path("ws")
        // The `ws()` filter will prepare Websocket handshake...
        .and(warp::ws())
        .and(users)
        .and(games)
        .map(|ws: warp::ws::Ws, users, games| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| user_connected(socket, users, games))
        });

    // GET / -> index html
    let index = warp::path::end().and(warp::fs::dir("static"));
    let static_dir = warp::path("static").and(warp::fs::dir("static"));

    let routes = index.or(ws).or(static_dir);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn user_connected(ws: WebSocket, users: Users, games: Games) {
    // Use a counter to assign a new unique ID for this user.
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new chat user: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn(rx.forward(user_ws_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("websocket send error: {}", e);
        }
    }));

    // Save the sender in our list of connected users.
    users.write().await.insert(my_id, tx.clone());

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    // Make an extra clone to give to our disconnection handler...
    let users2 = users.clone();
    let games2 = games.clone();

    // Every time the user sends a message, broadcast it to
    // all other users...
    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error(uid={}): {}", my_id, e);
                break;
            }
        };

        // Pattern match a user logging in vs a host wanting a new game
        // Skip any non-Text messages...
        if let Ok(s) = msg.to_str() {
            match serde_json::from_str(s).unwrap() {
                GameLobbyRequest::HostNewGame {
                    user_name,
                    user_type,
                } => {
                    // Generate a room code
                    // Add host to game

                    //let mut game_lobby_state = GameLobbyState::default();
                    let room_code = generate_room_code();

                    games
                        .write()
                        .await
                        .entry(room_code.clone())
                        .or_insert(GameLobbyState::default());

                    games
                        .write()
                        .await
                        .entry(room_code.clone())
                        .and_modify(|e| {
                            e.users.push(UserServerSideState {
                                user_id: my_id,
                                user_name: user_name,
                                user_type: user_type,
                                connected: true,
                                channel: Some(tx.clone()),
                            })
                        });

                    info!("New host creating game. Room code: {}", &room_code);

                    let msg = format!("Room code is: {}", &room_code);
                    if let Err(_disconnected) = tx.send(Ok(Message::text(&msg))) {
                        // The tx is disconnected, our `user_disconnected` code
                        // should be happening in another task, nothing more to
                        // do here.
                    }

                    info!("Host joining game lobby");

                    //game_lobby(my_id, login_info, &users, &games).await
                    game_lobby(my_id, &games, &room_code).await
                    // start_game()
                }

                GameLobbyRequest::UserLogin {
                    user_name,
                    user_type,
                    room_code,
                } => {
                    info!("New user joining room {}", &room_code.to_uppercase());

                    // Check for the existence of the room code in games
                    // if it doesn't exist, then send a message back to the user and then close the channel...
                    if !games.read().await.contains_key(&room_code.to_uppercase()) {
                        let error_msg =
                            format!("Room code: {} does not exist", &room_code.to_uppercase());

                        eprintln!("{}", &error_msg);

                        if let Err(_disconnected) = tx.send(Ok(Message::text(&error_msg))) {
                            // The tx is disconnected, our `user_disconnected` code
                            // should be happening in another task, nothing more to
                            // do here.
                        }

                        break;
                    } else {
                        // otherwise, add the user to the game room

                        info!("Adding new user into game room");

                        games
                            .write()
                            .await
                            .entry(room_code.clone())
                            .and_modify(|e| {
                                e.users.push(UserServerSideState {
                                    user_id: my_id,
                                    user_name: user_name,
                                    user_type: user_type,
                                    connected: true,
                                    channel: Some(tx.clone()),
                                })
                            });
                    }

                    info!("Joining the game lobby");

                    //game_lobby(my_id, login_info, &users, &games).await
                    game_lobby(my_id, &games, &room_code).await
                    // start_game()
                }

                GameLobbyRequest::HostStartGame { room_code } => {
                    println!("Start game for room: {:?}", &room_code);
                    game_start(&games, room_code).await
                }

                GameLobbyRequest::PlayerHand {
                    user_name,
                    room_code,
                    hand,
                } => {
                    println!("({}) {} played hand: {:?}", room_code, user_name, hand);
                    server_play_hand(&games, room_code, user_name).await
                }
            }
        } else {
            return;
        };
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(my_id, &users2, &games2).await;
}

async fn server_play_hand(games: &Games, room_code: String, user_name: String) {
    let random_hand: RPSHand = rand::random();

    for (room, game_state) in games.read().await.iter() {
        let all_users: Vec<String> = game_state
            .users
            .iter()
            .map(|u| u.user_name.clone())
            .collect();

        if room == &room_code {
            eprintln!("Hopefully about to print to all users in room?");
            for u in game_state.users.iter() {
                if u.user_name == user_name {
                    eprintln!("User: {:?}", u);

                    let resp = GameLobbyResponse::ServerHand {
                        hand: random_hand.clone(),
                    };

                    let msg = format!("{}", json!(resp).to_string());

                    if let Some(tx) = u.channel.clone() {
                        if let Err(_disconnected) = tx.send(Ok(Message::text(msg))) {}
                    }
                }
            }
        }
    }
}

async fn game_start(games: &Games, room_code: String) {
    games
        .write()
        .await
        .entry(room_code.clone())
        .and_modify(|e| {
            e.game_started = true;
        });

    for (room, game_state) in games.read().await.iter() {
        let all_users: Vec<String> = game_state
            .users
            .iter()
            .map(|u| u.user_name.clone())
            .collect();

        if room == &room_code {
            eprintln!("Hopefully about to print to all users in room?");
            for u in game_state.users.iter() {
                eprintln!("User: {:?}", u);

                let resp = GameLobbyResponse::GameStart {
                    room_code: room_code.clone(),
                };

                let msg = format!("{}", json!(resp).to_string());

                if let Some(tx) = u.channel.clone() {
                    if let Err(_disconnected) = tx.send(Ok(Message::text(msg))) {}
                }
            }
        }
    }

    // Humans vs the server
    // Best out of 5
    // Send updates to everyone on the number of turns taken + wins

    // When everyone has completed their turns. Announce winner(s), and allow the host to restart or end
}

async fn game_lobby(_my_id: usize, games: &Games, room_code: &String) {
    // TODO: Remove this when I've got all the other login stuff established
    //let new_msg = format!("<User#{}>: ", my_id);

    //eprintln!("{}", &new_msg);

    // loop over the game lobby's set of users and announce the current set of users
    for (room, game_state) in games.read().await.iter() {
        eprintln!("({}) Looping through room: {}", room_code, room);

        let all_users: Vec<String> = game_state
            .users
            .iter()
            .map(|u| u.user_name.clone())
            .collect();

        if room == room_code {
            eprintln!("Hopefully about to print to all users in room?");
            for u in game_state.users.iter() {
                eprintln!("User: {:?}", u);

                let resp = GameLobbyResponse::PartyUpdate {
                    room_code: room.to_string(),
                    users: all_users.clone(),
                };
                let msg = format!("{}", json!(resp).to_string());

                if let Some(tx) = u.channel.clone() {
                    if let Err(_disconnected) = tx.send(Ok(Message::text(msg))) {}
                }
            }
        }
    }
}

async fn user_disconnected(my_id: usize, users: &Users, games: &Games) {
    eprintln!("good bye user: {}", my_id);

    // If the user disconnected is a game host, close the streams for all users in the game
    // Otherwise, go into game and change toggle connected state, drop channel, change id to 0

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
}

pub fn generate_room_code() -> String {
    let mut rng = thread_rng();

    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric).to_ascii_uppercase())
        .filter(|c| c.is_alphabetic())
        .take(4)
        .collect()
}
