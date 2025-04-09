use clap::{value_parser, Arg, Command};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct ControlBlock {
    pub program_path: PathBuf,
    pub config_file_name: String,
    pub config: Value,

    pub m: usize,
    pub n: usize,
    pub stats_freq: usize,
    pub plot_freq: usize,
    pub px: usize,
    pub py: usize,
    pub niters: usize
}

impl ControlBlock {
    pub fn new(args: Vec<String>) -> Self {
        let matches = Command::new("controlblock")
            .arg(Arg::new("config").short('c').help("config file name"))
            .arg(Arg::new("n").short('n').value_parser(value_parser!(usize)))
            .arg(
                Arg::new("niters")
                    .short('i')
                    .value_parser(value_parser!(usize)),
            )
            .arg(
                Arg::new("stats-freq")
                    .short('s')
                    .value_parser(value_parser!(usize)),
            )
            .arg(
                Arg::new("plot")
                    .short('p')
                    .value_parser(value_parser!(usize)),
            )
            .arg(Arg::new("px").short('x').value_parser(value_parser!(usize)))
            .arg(Arg::new("py").short('y').value_parser(value_parser!(usize)))
            .arg(
                Arg::new("nocomm")
                    .short('k')
                    .action(clap::ArgAction::SetTrue),
            )
            .get_matches_from(args);
        let program_path = std::env::current_exe().unwrap();
        let config_file_name = matches.get_one::<String>("config").unwrap().to_string();
        let project_root = std::env::current_dir().unwrap();
        let absolute_file_path = project_root.join(&config_file_name);        
        let mut m = 100;
        let mut n = 100;
        let mut stats_freq = 0;
        let mut plot_freq = 0;
        let mut px = 1;
        let mut py = 1;
        let mut niters = 100;
        let config: Value = match fs::read_to_string(&absolute_file_path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| Value::Null),
            Err(_) => Value::Null,
        };

        if let Some(config_obj) = config.as_object() {
            if let Some(val) = config_obj.get("-n") {
                if let Some(v) = val.as_u64() {
                    n = v as usize;
                    m = n;
                }
            }
            if let Some(val) = config_obj.get("-i") {
                if let Some(v) = val.as_u64() {
                    niters = v as usize;
                }
            }
            if let Some(val) = config_obj.get("-x") {
                if let Some(v) = val.as_u64() {
                    px = v as usize;
                }
            }
            if let Some(val) = config_obj.get("-y") {
                if let Some(v) = val.as_u64() {
                    py = v as usize;
                }
            }
        }
        if matches.contains_id("n") {
            n = *matches.get_one("n").unwrap();
            m = n;
        }
        if matches.contains_id("niters") {
            niters = *matches.get_one("niters").unwrap();
        }
        if matches.contains_id("stats-freq") {
            stats_freq = *matches.get_one("stats-freq").unwrap();
        }
        if matches.contains_id("plot") {
            plot_freq = *matches.get_one("plot").unwrap();
        }

        if matches.contains_id("px") {
            px = *matches.get_one("px").unwrap();
        }
        if matches.contains_id("py") {
            py = *matches.get_one("py").unwrap();
        }

        ControlBlock {
            program_path,
            config_file_name,
            config,
            m,
            n,
            stats_freq,
            plot_freq,
            px,
            py,
            niters
        }
    }
}
