use std::env;
use std::fs::File;
use std::io::Read;

use network_commons::error::CommonError;
use twamp::{Twamp, TwampConfiguration, TwampResult};
use validator::Validate;

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

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: program_name config_file_path");
        return;
    }

    let config_file = &args[1];
    let mut file = File::open(config_file).expect("failed to open config file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("failed to read config file");

    let config: TwampConfiguration =
        serde_json::from_str(&contents).expect("failed to parse config");

    config.validate().expect("invalid configuration");

    let app = App::new(config);
    let _ = app.run();
}
