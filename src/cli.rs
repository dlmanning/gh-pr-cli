use clap::{Parser, ValueEnum};
use octocrab::params::State;

#[derive(Debug, Clone, Copy)]
pub enum ArgState {
    Open,
    Closed,
    All,
}

impl ValueEnum for ArgState {
    fn from_str(input: &str, _: bool) -> Result<Self, String> {
        match input {
            "open" => Ok(ArgState::Open),
            "closed" => Ok(ArgState::Closed),
            "all" => Ok(ArgState::All),
            _ => Err(clap::Error::new(clap::error::ErrorKind::ValueValidation).to_string()),
        }
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(match self {
            ArgState::Open => "open",
            ArgState::Closed => "closed",
            ArgState::All => "all",
        }))
    }

    fn value_variants<'a>() -> &'a [Self] {
        &[ArgState::Open, ArgState::Closed, ArgState::All]
    }
}

impl Into<State> for ArgState {
    fn into(self) -> State {
        match self {
            ArgState::Open => State::Open,
            ArgState::Closed => State::Closed,
            ArgState::All => State::All,
        }
    }
}

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[clap(short = 'r', long = "repo")]
    pub repo: String,
    #[clap(short = 'c', long = "comments", default_value = "false")]
    pub comments: bool,
    #[clap(short = 's', long = "state", default_value = "open")]
    pub state: ArgState,
    // Flag for last n PR's to show
    #[clap(short = 'l', long = "last", default_value = "50")]
    pub last: u8,
}

impl Cli {
    pub fn get_args() -> Self {
        Self::parse()
    }
}
