use std::fmt;
use std::path::PathBuf;

use clap::Parser;

/// The Result type for autorandr.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The hidden code for this iteration of the game.
    #[arg(short, long, value_name = "HIDDEN_CODE")]
    hidden_code: String,
}

enum Color {
    Red,
    Green,
    Blue,
    Purple,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = match self {
            Color::Red => 'r',
            Color::Green => 'g',
            Color::Blue => 'b',
            Color::Purple => 'p',
        };
        write!(f, "{}", c)
    }
}

struct Code(Color, Color, Color, Color);

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {}", self.0, self.1, self.2, self.3)
    }
}

impl From<String> for Code {
    fn from(s: String) -> Self {
        Self(Color::Red, Color::Red, Color::Red, Color::Red)
    }
}

struct Board {
    hidden_code: Code,
    rounds: Vec<Round>,
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for round in &self.rounds {
            write!(f, "{}", round)?;
        }
        Ok(())
    }
}

enum Key {
    ColorCorrect,
    ColorAndPositionCorrect,
    Empty,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = match self {
            Key::ColorCorrect => 'w',
            Key::ColorAndPositionCorrect => 'b',
            Key::Empty => ' ',
        };
        write!(f, "{}", c)
    }
}

struct Score(Key, Key, Key, Key);

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {}", self.0, self.1, self.2, self.3)
    }
}

struct Round {
    input_code: Code,
    score: Score,
}

impl fmt::Display for Round {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} - {}", self.input_code, self.score)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut board = Board {
        hidden_code: cli.hidden_code.into(),
        rounds: Vec::new(),
    };

    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer);
    println!("{}", board);
    return Ok(());
}
