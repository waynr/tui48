use std::collections::HashSet;
use std::fmt;
use std::io::Write;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    style::Stylize,
    terminal,
};

/// The Result type for autorandr.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Eq, PartialEq, Hash)]
enum Color {
    Red,
    Green,
    Blue,
    Yellow,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = match self {
            Color::Red => "r".red(),
            Color::Green => "g".green(),
            Color::Blue => "b".blue(),
            Color::Yellow => "y".yellow(),
        };
        write!(f, "{}", c)
    }
}

#[derive(Clone)]
struct Code {
    positional: [Color; 4],
    set: HashSet<Color>,
}

impl Code {
    fn score(&self, other: Code) -> Score {
        let score: Vec<ScoreDetail> = self
            .positional
            .iter()
            .zip(other.positional.iter())
            .map(|(s, o)| {
                if s == o {
                    ScoreDetail::ColorAndPositionCorrect
                } else if self.set.contains(o) {
                    ScoreDetail::ColorCorrect
                } else {
                    ScoreDetail::Empty
                }
            })
            .collect();
        Score(
            score[0].clone(),
            score[1].clone(),
            score[2].clone(),
            score[3].clone(),
        )
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.positional[0], self.positional[1], self.positional[2], self.positional[3]
        )
    }
}

impl TryFrom<String> for Code {
    type Error = Box<dyn std::error::Error>;

    fn try_from(s: String) -> Result<Self> {
        let mut set = HashSet::new();
        let pos: Vec<Color> = s
            .chars()
            .filter_map(|c| match c {
                '(' | ')' | ',' => None,
                'r' => {
                    set.insert(Color::Red);
                    Some(Color::Red)
                }
                'g' => {
                    set.insert(Color::Green);
                    Some(Color::Green)
                }
                'b' => {
                    set.insert(Color::Blue);
                    Some(Color::Blue)
                }
                'y' => {
                    set.insert(Color::Yellow);
                    Some(Color::Yellow)
                }
                _ => None,
            })
            .collect();
        match pos.len() {
            x if x < 4 => return Err(String::from("not enough characters").into()),
            x if x > 4 => return Err(String::from("too many characters").into()),
            _ => (),
        }
        Ok(Self {
            positional: [
                pos[0].clone(),
                pos[1].clone(),
                pos[2].clone(),
                pos[3].clone(),
            ],
            set,
        })
    }
}

struct Board {
    hidden_code: Code,
    rounds: Vec<Round>,
}

impl Board {
    fn get_input(&mut self) -> Result<bool> {
        println!("{}", &self);
        let mut buffer = String::new();

        print!("guess: ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut buffer)?;

        let code: Code = buffer.try_into()?;
        let round = Round {
            input_code: code.clone(),
            score: self.hidden_code.score(code),
        };

        self.rounds.push(round);

        match self.rounds.last() {
            Some(rs) => Ok(rs.wins()),
            None => Ok(false),
        }
    }

    fn init() -> Result<Self> {
        let mut buffer = String::new();
        println!(" to begin you will need to input hidden code.");
        println!(
            " codes can be one of four letters:\n {} {} {} {}",
            "r".red(),
            "g".green(),
            "b".blue(),
            "y".yellow()
        );
        print!("hidden code: ");
        std::io::stdout().flush()?;

        terminal::enable_raw_mode()?;
        while let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Enter => {
                    break;
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                }
                _ => {}
            }
        }
        terminal::disable_raw_mode()?;

        println!("\n great.\n");
        println!(
            r#"score is represented with three different colors:\n
 correct color, correct position: {}
 correct color, wrong position: {}
 wrong color, wrong position: {}
 good luck!"#,
            " ".on_cyan(),
            " ".on_white(),
            " ".on_red(),
        );

        Ok(Self {
            hidden_code: buffer.try_into()?,
            rounds: Vec::new(),
        })
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines = Vec::new();
        for round in &self.rounds {
            let s = format!("| {} |", round);
            lines.push(s);
        }
        if self.rounds.len() > 0 {
            write!(f, "\n{}\n", "=".repeat(21))?;
            write!(f, "{}\n", lines.join("\n"))?;
            write!(f, "{}\n", "=".repeat(21))?;
        }
        Ok(())
    }
}

#[derive(Clone)]
enum ScoreDetail {
    ColorCorrect,
    ColorAndPositionCorrect,
    Empty,
}

impl fmt::Display for ScoreDetail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = match self {
            ScoreDetail::ColorCorrect => " ".on_white(),
            ScoreDetail::ColorAndPositionCorrect => " ".on_cyan(),
            ScoreDetail::Empty => " ".on_red(),
        };
        write!(f, "{}", c)
    }
}

struct Score(ScoreDetail, ScoreDetail, ScoreDetail, ScoreDetail);

impl Score {
    fn wins(&self) -> bool {
        match self {
            Score(
                ScoreDetail::ColorAndPositionCorrect,
                ScoreDetail::ColorAndPositionCorrect,
                ScoreDetail::ColorAndPositionCorrect,
                ScoreDetail::ColorAndPositionCorrect,
            ) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {}", self.0, self.1, self.2, self.3)
    }
}

struct Round {
    input_code: Code,
    score: Score,
}

impl Round {
    fn wins(&self) -> bool {
        self.score.wins()
    }
}

impl fmt::Display for Round {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} | {}", self.input_code, self.score)
    }
}

fn main() -> Result<()> {
    let mut board = Board::init()?;

    loop {
        if board.get_input()? {
            println!("{}", board);
            println!("congratulations, you win!");
            break;
        }
    }
    return Ok(());
}
