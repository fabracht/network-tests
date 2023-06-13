use crate::twamp::Twamp;
use crate::twamp_light_sender::result::TwampResult;

use ::common::error::CommonError;
use twamp::TwampConfiguration;
use validator::Validate;
mod common;
mod twamp;
mod twamp_control;
mod twamp_light_reflector;
mod twamp_light_sender;

use clap::Parser;
use std::fs::File;
use std::io::Read;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(
        short,
        long,
        default_value_t = String::from("twamp/configurations/receiver_config.json")
    )]
    config_file: String,
}

#[derive(Debug)]
struct App {
    config: TwampConfiguration,
}

impl App {
    fn new(config: TwampConfiguration) -> Self {
        Self { config }
    }

    fn run(&self) -> Result<(), CommonError> {
        log::debug!("{:?}", self.config);
        let twamp = Twamp::new(self.config.clone());
        let result = match twamp.generate() {
            Ok(mut strategy) => match strategy.execute() {
                Ok(result) => result,
                Err(e) => TwampResult {
                    session_results: vec![],
                    error: Some(e.to_string()),
                },
            },
            Err(e) => TwampResult {
                session_results: vec![],
                error: Some(e.to_string()),
            },
        };

        log::info!(
            "Result {:#}",
            serde_json::to_string_pretty(&result).unwrap()
        );

        Ok(())
    }
}

fn main() {
    let _ = log4rs::init_file("twamp/log_config.yml", Default::default());

    let args = Cli::parse();

    let mut file = File::open(args.config_file).expect("failed to open config file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("failed to read config file");

    let config: TwampConfiguration =
        serde_json::from_str(&contents).expect("failed to parse config");

    config.validate().expect("invalid configuration");

    let app = App::new(config);
    let _ = app.run();
}
