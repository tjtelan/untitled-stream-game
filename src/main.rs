mod client;
mod game;
mod server;

use std::io::Error as IoError;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ServerOptions {
    #[structopt(long, default_value = "127.0.0.1:12345")]
    pub server_listen_addr: String,
}

#[derive(Debug, StructOpt)]
pub struct ServerRoomCode {
    room_code: String,
}

#[derive(Debug, StructOpt)]
pub enum ClientMode {
    Host,
    Join(ServerRoomCode),
}

#[derive(Debug, StructOpt)]
pub struct ClientOptions {
    #[structopt(long, default_value = "ws://127.0.0.1:12345")]
    pub server_addr: String,
    #[structopt(subcommand)]
    pub client_mode: ClientMode,
}

#[derive(Debug, StructOpt)]
enum AppMode {
    Server(ServerOptions),
    Client(ClientOptions),
}

#[derive(Debug, StructOpt)]
struct ApplicationArguments {
    #[structopt(flatten)]
    pub app_mode: AppMode,
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let args = ApplicationArguments::from_args();

    let exit_result = match args.app_mode {
        AppMode::Server(server_opts) => {
            println!("Start server on: {:?}", server_opts.server_listen_addr);
            server::start_server(server_opts).await;
        }
        AppMode::Client(client_opts) => {
            // TODO: Handle host vs join mode. ALL_CAPS the room code
            println!(
                "Connect to server in {:?} mode at: {:?}",
                client_opts.client_mode, client_opts.server_addr
            );
            client::client_connect(client_opts).await;
        }
    };

    Ok(exit_result)
}
