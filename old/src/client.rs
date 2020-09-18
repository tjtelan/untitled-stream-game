use std::env;
use std::str;
//use serde_json

use futures_util::{future, pin_mut, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use std::sync::{Arc, Mutex};

use crate::game;
use crate::{ClientMode, ClientOptions, ServerRoomCode};

pub async fn client_connect(opts: ClientOptions) {
    let connect_addr = opts.server_addr.clone();

    let url = url::Url::parse(&connect_addr).unwrap();

    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();

    let client_state = match opts.client_mode {
        ClientMode::Host => game::GamePlayer {
            user_name: opts.player_name,
            user_type: game::UserMode::GameHost,
            room_code: None,
            peer_addr: None,
        },
        ClientMode::Join(ref room_code) => game::GamePlayer {
            user_name: opts.player_name,
            user_type: game::UserMode::GamePlayer,
            room_code: Some(room_code.room_code.clone()),
            peer_addr: None,
        },
    };

    let client_state_async = Arc::new(Mutex::new(client_state));

    tokio::spawn(read_stdin(client_state_async.clone(), stdin_tx));

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (write, read) = ws_stream.split();

    let stdin_to_ws = stdin_rx.map(Ok).forward(write);

    // From the server
    let ws_to_stdout = {
        read.for_each(|message| async {
            let incoming_data = message.unwrap().into_data();

            //let incoming_data =
            //    serde_json::from_str::<game::GamePlayer>(str::from_utf8(&incoming_data).unwrap())
            //        .unwrap();

            match serde_json::from_str::<game::GameMessageType>(
                str::from_utf8(&incoming_data).unwrap(),
            )
            .unwrap()
            {
                game::GameMessageType::NewPlayer(data) => {
                    println!("incoming_data: {:?}", data);

                    let mut state = client_state_async.lock().unwrap();

                    // This is intended to only be set when a client initially connects
                    if *state.user_name == data.user_name {
                        println!("Setting data from server");
                        *state = data;
                    }
                }
                game::GameMessageType::AllPlayers(all_players) => {
                    println!("allplayers: {:?}", all_players)
                }
            }
        })
    };

    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;
}

// TODO: Rename read_stdin to something more meaningful in our case
// Our helper method which will read data from stdin and send it along the
// sender provided.
async fn read_stdin(
    client: Arc<Mutex<game::GamePlayer>>,
    tx: futures_channel::mpsc::UnboundedSender<Message>,
) {
    let mut stdin = tokio::io::stdin();

    {
        let init_connect = client.lock().unwrap();

        if init_connect.room_code.is_none() || init_connect.peer_addr.is_none() {
            println!("Prompt server to return server-state info");
            tx.unbounded_send(Message::binary(
                serde_json::to_string(&game::GameMessageType::NewPlayer(init_connect.clone()))
                    .unwrap(),
            ))
            .unwrap();
        }

        drop(init_connect);
    }

    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);
        tx.unbounded_send(Message::binary(
            serde_json::to_string(&&game::GameMessageType::NewPlayer(
                client.lock().unwrap().clone(),
            ))
            .unwrap(),
        ))
        .unwrap();
    }
}
