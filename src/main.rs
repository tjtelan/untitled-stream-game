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

fn main() {
    let player1 = Player {
        id: 1,
        hand: RPSOptions::Rock,
    };
    let player2 = Player {
        id: 2,
        hand: RPSOptions::Rock,
    };

    println!("The winner is: {:?}", rps_winner(player1, player2));
}

fn rps_winner(p1: Player, p2: Player) -> Player {
    match (p1.hand.clone(), p2.hand.clone()) {
        (RPSOptions::Rock, RPSOptions::Rock)
        | (RPSOptions::Paper, RPSOptions::Paper)
        | (RPSOptions::Scissors, RPSOptions::Scissors) => Player {
            id: 0,
            hand: RPSOptions::Rock,
        },

        (RPSOptions::Rock, RPSOptions::Paper) => p2,
        (RPSOptions::Rock, RPSOptions::Scissors) => p1,

        (RPSOptions::Paper, RPSOptions::Rock) => p1,
        (RPSOptions::Paper, RPSOptions::Scissors) => p2,

        (RPSOptions::Scissors, RPSOptions::Rock) => p2,
        (RPSOptions::Scissors, RPSOptions::Paper) => p1,
    }
}
