use crate::twamp::Twamp;

use ::common::error::CommonError;
use twamp::TwampConfiguration;
use validator::Validate;
mod common;
mod twamp;
mod twamp_light_reflector;
mod twamp_light_sender;

use std::fs::File;
use std::io::Read;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "myapp")]
struct Cli {
    #[structopt(short = "c", long = "config", default_value = "config.json")]
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
        // your app logic goes here
        log::info!("{:?}", self.config);
        let twamp = Twamp::new(self.config.clone());
        let mut strategy = twamp.generate()?;
        let result = strategy.execute()?;
        // let packet_results = result.session.packets.clone().unwrap();
        log::info!(
            "Result {:#}",
            serde_json::to_string_pretty(&result).unwrap()
        );
        // _calculate_offsets(&packet_results);

        Ok(())
    }
}

fn main() {
    log4rs::init_file("twamp/log_config.yml", Default::default()).unwrap();

    let args = Cli::from_args();

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
