use encoding;
use loirc;
use loirc::Message;
use loirc::Prefix::{Server, User};
use serde::Deserialize;
use serde_yaml;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::Duration;

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
            channel: "#goaste".to_string(),
            xchannels: vec!["#goaste2".to_string()],
            password: None,
            debug: false,
            port: 6667,
            delay: "2s".to_string(),
            colors: HashMap::from([
                ("origin".to_string(), "pink".to_string()),
                ("title".to_string(), "bold".to_string()),
                ("hash".to_string(), "lightgrey".to_string()),
                ("link".to_string(), "lightblue".to_string()),
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
            maxage: "1h".to_string(),
            frequency: "10m".to_string(),
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

fn handle_irc_messages(gruik_config: &GruikConfig, irc_writer: &loirc::Writer, msg: Message) {
    /*
     * PING
     */
    if msg.code == loirc::Code::Ping {
        let ping_arg = match msg.args.get(0) {
            Some(r) => r,
            None => {
                println!("Can't get ping argument! exiting.");
                std::process::exit(1);
            }
        };
        if let Err(e) = irc_writer.raw(format!("PONG :{}\n", ping_arg)) {
            println!("Couldn't send the 'PONG' command{:?}", e);
        }
        return;
    }
    /*
     * RPL_WELCOME
     */
    if msg.code == loirc::Code::RplWelcome {
        if let Err(e) = irc_writer.raw(format!("JOIN {}\n", gruik_config.irc.channel)) {
            println!("Couldn't send the 'JOIN' command{:?}", e);
        }
        return;
    }
    /*
     * PRIVMSG
     */
    if msg.code == loirc::Code::Privmsg {
        let empty_str = "".to_string();
        let msg_target = msg.args.get(0).unwrap_or(&empty_str);
        let msg_str = msg.args.get(1).unwrap_or(&empty_str);

        /*
         * !lsfeeds
         */
        if msg_str.starts_with("!lsfeeds") {
            println!("NOT IMPLEMENTED: !lsfeeds");
        }
        /*
         * !xpost
         */
        else if msg_str.starts_with("!xpost") {
            println!("NOT IMPLEMENTED: !xpost");
        }
        /*
         * !latest
         */
        else if msg_str.starts_with("!latest") {
            println!("NOT IMPLEMENTED: !latest");
        }

        // All commands below requires OP
        let msg_source: String = match msg.prefix {
            Some(p) => match p {
                User(u) => u.nickname,
                Server(s) => s,
            },
            None => "".to_string(),
        };
        if !gruik_config.irc.ops.contains(&msg_source) {
            return;
        }

        /*
         * !die
         */
        if msg_str.starts_with("!die") {
            println!("NOT IMPLEMENTED: !die");
        }
        /*
         * !addfeed
         */
        else if msg_str.starts_with("!addfeed") {
            println!("NOT IMPLEMENTED: !addfeed");
        }
        /*
         * !rmfeed
         */
        else if msg_str.starts_with("!rmfeed") {
            println!("NOT IMPLEMENTED: !rmfeed");
        }

        return;

        // We discard all other messages
    }
}

fn handle_irc_events(
    gruik_config: &GruikConfig,
    irc_writer: &loirc::Writer,
    irc_reader: &loirc::Reader,
) {
    for event in irc_reader.iter() {
        if gruik_config.irc.debug {
            dbg!(&event);
        }
        match event {
            loirc::Event::Message(msg) => {
                handle_irc_messages(gruik_config, irc_writer, msg);
            }
            _ => {
                println!("Don't know what to do with the following event :");
                dbg!(event);
            }
        }
    }
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

    let gruik_config: GruikConfig = match serde_yaml::from_str(&yaml) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't parse '{config_filename}' : {e}");
            std::process::exit(1);
        }
    };

    let (irc_writer, irc_reader) = match loirc::connect(
        format!("{}:{}", gruik_config.irc.server, gruik_config.irc.port),
        loirc::ReconnectionSettings::Reconnect {
            max_attempts: 10,
            delay_between_attempts: Duration::from_secs(2),
            delay_after_disconnect: Duration::from_secs(2),
        },
        encoding::all::UTF_8,
    ) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't connect to IRC server : {e}");
            std::process::exit(1);
        }
    };

    // register
    if let Err(e) = irc_writer.raw(format!("NICK {}\n", &gruik_config.irc.nick)) {
        println!("Can't send the 'NICK' command : {:?}", e);
        std::process::exit(1);
    }

    if let Err(e) = irc_writer.raw(format!(
        "USER {} 0 * :{}\n",
        &gruik_config.irc.nick, &gruik_config.irc.nick
    )) {
        println!("Can't send the 'USER' command : {:?}", e);
        std::process::exit(1);
    }
    // *Warning*, this is a *blocking* function!
    handle_irc_events(&gruik_config, &irc_writer, &irc_reader);
}
