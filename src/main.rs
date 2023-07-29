use serde::Deserialize;
use serde_yaml;
use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct IrcConfig {
    server: String,
    nick: String,
    channel: String,
    xchannels: Vec<String>,
    password: Option<String>,
    debug: bool,
    port: u16,
    delay: String,
    colors: HashMap<String, String>,
    ops: Vec<String>,
}

impl Default for IrcConfig {
    fn default() -> Self {
        IrcConfig {
            server: "irc.libera.chat".to_string(),
            nick: "gruik".to_string(),
            channel: "goaste".to_string(),
            xchannels: vec!["goaste2".to_string()],
            password: None,
            debug: false,
            port: 6667,
            delay: "2s".to_string(),
            colors: HashMap::from([
                (String::from("origin"), String::from("pink")),
                (String::from("title"), String::from("bold")),
                (String::from("hash"), String::from("lightgrey")),
                (String::from("link"), String::from("lightblue")),
            ]),
            ops: vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct FeedsConfig {
    urls: Vec<String>,
    maxnews: u16,
    maxage: String,
    frequency: String,
    ringsize: u16,
}

impl Default for FeedsConfig {
    fn default() -> Self {
        FeedsConfig {
            urls: vec![],
            maxnews: 10,
            maxage: String::from("1h"),
            frequency: String::from("10m"),
            ringsize: 100,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GruikConfig {
    irc: IrcConfig,
    feeds: FeedsConfig,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let config_filename = match args.get(1) {
        Some(s) => s,
        None => "config.yaml",
    };

    let yaml = match fs::read_to_string(config_filename) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't read '{config_filename}' : {e}");
            std::process::exit(1);
        }
    };

    let gruik_config: GruikConfig = serde_yaml::from_str(&yaml).unwrap();
}
