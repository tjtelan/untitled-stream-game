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
//use serde_json::Value;

use log::info;

use rand::{distributions::Alphanumeric, thread_rng, Rng};

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

type Games = Arc<RwLock<HashMap<String, Vec<UserServerSideState>>>>;

#[derive(Deserialize, Serialize, Debug)]
struct GameLobbyResponse {
    room_code: String,
    users: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
enum ClientMsgRequest {
    UserLogin(UserReq),
    HostNewGame(HostReq),
    //StartGame(UserReq)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
enum UserType {
    Host,
    Player,
}

#[derive(Deserialize, Serialize, Debug)]
struct HostReq {
    user_name: String,
    user_type: UserType,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct UserReq {
    user_name: String,
    user_type: UserType,
    room_code: String,
}

#[derive(Debug, Clone)]
struct UserServerSideState {
    user_id: usize,
    user_name: String,
    user_type: UserType,
    //room_code: String,
    connected: bool,
    channel: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>,
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

    let mut room_code = String::new();

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
                ClientMsgRequest::HostNewGame(login_info) => {
                    // Generate a room code
                    // Add host to game

                    room_code = generate_room_code();

                    games
                        .write()
                        .await
                        .entry(room_code.clone())
                        .or_insert(Vec::new());

                    games
                        .write()
                        .await
                        .entry(room_code.clone())
                        .and_modify(|e| {
                            e.push(UserServerSideState {
                                user_id: my_id,
                                user_name: login_info.user_name,
                                user_type: login_info.user_type,
                                connected: true,
                                channel: Some(tx.clone()),
                            })
                        });
                    //games.write().await.insert(room_code.clone(), Vec::new());

                    //games
                    //    .write()
                    //    .await
                    //    .get_mut(&room_code)
                    //    .unwrap()
                    //    .push(UserServerSideState {
                    //        user_id: my_id,
                    //        user_name: login_info.user_name,
                    //        user_type: login_info.user_type,
                    //        connected: true,
                    //        channel: Some(tx.clone()),
                    //    });

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

                ClientMsgRequest::UserLogin(login_info) => {
                    info!(
                        "New user joining room {}",
                        &login_info.room_code.to_uppercase()
                    );

                    // Check for the existence of the room code in games
                    // if it doesn't exist, then send a message back to the user and then close the channel...
                    if !games
                        .read()
                        .await
                        .contains_key(&login_info.room_code.to_uppercase())
                    {
                        let error_msg = format!(
                            "Room code: {} does not exist",
                            &login_info.room_code.to_uppercase()
                        );

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

                        // TODO: What I should do it look for a user of the same name first, and modify that record. Otherwise push new
                        //if let Some(u) = games
                        //    .write()
                        //    .await
                        //    .get_mut(&login_info.room_code.to_uppercase())
                        //{
                        //    u.push(UserServerSideState {
                        //        user_id: my_id,
                        //        user_name: login_info.clone().user_name,
                        //        user_type: login_info.clone().user_type,
                        //        connected: true,
                        //        channel: Some(tx.clone()),
                        //    })
                        //}

                        room_code = login_info.room_code.clone();

                        games
                            .write()
                            .await
                            .entry(room_code.clone())
                            .and_modify(|e| {
                                e.push(UserServerSideState {
                                    user_id: my_id,
                                    user_name: login_info.clone().user_name,
                                    user_type: login_info.clone().user_type,
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
            }
        } else {
            return;
        };
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(my_id, &users2, &games2).await;
}

async fn game_lobby(my_id: usize, games: &Games, room_code: &String) {
    // TODO: Remove this when I've got all the other login stuff established
    //let new_msg = format!("<User#{}>: ", my_id);

    //eprintln!("{}", &new_msg);

    // loop over the game lobby's set of users and announce the current set of users
    for (room, users) in games.read().await.iter() {

        eprintln!("({}) Looping through room: {}", room_code, room);

        let all_users : Vec<String> = users.iter().map(|u| u.user_name.clone()).collect();

        if room == room_code {

            eprintln!("Hopefully about to print to all users in room?");
            for u in users.iter() {

                eprintln!("User: {:?}", u);

                let msg = format!("{:?}", all_users);

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
