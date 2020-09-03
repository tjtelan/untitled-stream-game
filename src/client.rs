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
        ClientMode::Host => game::GameMessage {
            user_type: game::UserMode::GameHost,
            room_code: None,
        },
        ClientMode::Join(ref room_code) => game::GameMessage {
            user_type: game::UserMode::GamePlayer,
            room_code: Some(room_code.room_code.clone()),
        },
    };

    //let client_state_async = Arc::new(Mutex::new(client_state));

    tokio::spawn(read_stdin(client_state, stdin_tx));

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (write, read) = ws_stream.split();

    let stdin_to_ws = stdin_rx.map(Ok).forward(write);
    let ws_to_stdout = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();

            let data =
                serde_json::from_str::<game::GameMessage>(str::from_utf8(&data).unwrap()).unwrap();

            println!("data: {:?}", data);
            //tokio::io::stdout().write_all(&data).await.unwrap();
            //tokio::io::stdout().write_all(format!("{:?}", data).as_bytes()).await.unwrap();
        })
    };

    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;
}

// TODO: Rename read_stdin to something more meaningful in our case
// Our helper method which will read data from stdin and send it along the
// sender provided.
async fn read_stdin(client: game::GameMessage, tx: futures_channel::mpsc::UnboundedSender<Message>) {
    let mut stdin = tokio::io::stdin();
    //let mut game_msg = match client.client_mode {
    //    ClientMode::Host => game::GameMessage {
    //        user_type: game::UserMode::GameHost,
    //        room_code: None,
    //    },
    //    ClientMode::Join(ref room_code) => game::GameMessage {
    //        user_type: game::UserMode::GamePlayer,
    //        room_code: Some(room_code.room_code.clone()),
    //    },
    //};
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);
        //tx.unbounded_send(Message::binary(format!("{:?}", game_msg)))
        tx.unbounded_send(Message::binary(serde_json::to_string(&client).unwrap()))
            .unwrap();
    }
}
