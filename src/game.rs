use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::iter;

#[derive(Debug, Serialize, Deserialize)]
pub enum UserMode {
    GameHost,
    GamePlayer,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameMessage {
    pub user_type: UserMode,
    pub room_code: Option<String>,
}

pub fn generate_room_code() -> String {
    let mut rng = thread_rng();

    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric).to_ascii_uppercase())
        .filter(|c| c.is_alphabetic())
        .take(4)
        .collect()
}

#[derive(Debug, Clone)]
enum RPSOptions {
    Rock,
    Paper,
    Scissors,
}

#[derive(Debug)]
struct Player {
    id: u8,
    hand: RPSOptions,
}

//fn main() {
//    let player1 = Player {
//        id: 1,
//        hand: RPSOptions::Rock,
//    };
//    let player2 = Player {
//        id: 2,
//        hand: RPSOptions::Rock,
//    };
//
//    println!("The winner is: {:?}", rps_winner(player1, player2));
//}
//
//fn rps_winner(p1: Player, p2: Player) -> Player {
//    match (p1.hand.clone(), p2.hand.clone()) {
//        (RPSOptions::Rock, RPSOptions::Rock)
//        | (RPSOptions::Paper, RPSOptions::Paper)
//        | (RPSOptions::Scissors, RPSOptions::Scissors) => Player {
//            id: 0,
//            hand: RPSOptions::Rock,
//        },
//
//        (RPSOptions::Rock, RPSOptions::Paper) => p2,
//        (RPSOptions::Rock, RPSOptions::Scissors) => p1,
//
//        (RPSOptions::Paper, RPSOptions::Rock) => p1,
//        (RPSOptions::Paper, RPSOptions::Scissors) => p2,
//
//        (RPSOptions::Scissors, RPSOptions::Rock) => p2,
//        (RPSOptions::Scissors, RPSOptions::Paper) => p1,
//    }
//}
//
