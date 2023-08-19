use base16ct;
use chrono::{DateTime, Duration, Utc};
use duration_str::deserialize_duration_chrono;
use encoding;
use feed_rs;
use loirc;
use loirc::Message;
use loirc::Prefix::{Server, User};
use serde::{Deserialize, Serialize};
use serde_json;
use serde_yaml;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::{collections::HashMap, env, fs, sync::Arc, sync::Mutex, thread};
use ureq;

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
    #[serde(deserialize_with = "deserialize_duration_chrono")]
    delay: Duration,
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
            delay: Duration::seconds(2),
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
    #[serde(deserialize_with = "deserialize_duration_chrono")]
    maxage: Duration,
    #[serde(deserialize_with = "deserialize_duration_chrono")]
    frequency: Duration,
    ringsize: usize,
}

impl Default for FeedsConfig {
    fn default() -> Self {
        FeedsConfig {
            urls: vec![],
            maxnews: 10,
            maxage: Duration::hours(1),
            frequency: Duration::minutes(10),
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct News {
    origin: String,
    title: String,
    links: Vec<String>,
    date: DateTime<Utc>,
    hash: String,
}

fn handle_irc_messages(
    config: &GruikConfig,
    irc_writer: &loirc::Writer,
    msg: Message,
    news_list: &Arc<Mutex<VecDeque<News>>>,
) {
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
        if let Err(e) = irc_writer.raw(format!("JOIN {}\n", config.irc.channel)) {
            println!("Couldn't join {} : {:?}", config.irc.channel, e);
        }
        for channel in &config.irc.xchannels {
            if let Err(e) = irc_writer.raw(format!("JOIN {}\n", channel)) {
                println!("Couldn't join {} : {:?}", channel, e);
            }
        }
        return;
    }
    /*
     * PRIVMSG
     */
    if msg.code == loirc::Code::Privmsg {
        let empty_str = "".to_string();
        let msg_source: String = match msg.prefix {
            Some(p) => match p {
                User(u) => u.nickname,
                Server(s) => s,
            },
            None => "".to_string(),
        };
        let msg_str = msg.args.get(1).unwrap_or(&empty_str);
        let msg_args: Vec<&str> = msg_str.split(" ").collect();
        let (_, msg_args) = msg_args.split_at(1);

        /*
         * !lsfeeds
         */
        if msg_str.starts_with("!lsfeeds") {
            for (i, feed) in config.feeds.urls.iter().enumerate() {
                if let Err(e) = irc_writer.raw(format!(
                    "PRIVMSG {} {}\n",
                    &msg_source,
                    format!("{}. {}", i.to_string(), feed)
                )) {
                    println!("Failed to send an IRC message... ({:?})", e);
                } else {
                    thread::sleep(config.irc.delay.to_std().unwrap());
                }
            }
        }
        /*
         * !xpost
         */
        else if msg_str.starts_with("!xpost") {
            let hash = match msg_args.get(0) {
                None => "".to_string(),
                Some(h) => h.replace("#", ""),
            };

            for news in news_list.lock().unwrap().iter() {
                println!("{}", news.hash);
                if news.hash == hash {
                    for channel in &config.irc.xchannels {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {}\n",
                            &channel,
                            format!(
                                "{} (from {} on {})",
                                fmt_news(news),
                                msg_source,
                                config.irc.channel
                            )
                        )) {
                            println!("Failed to send an IRC message... ({:?})", e);
                        } else {
                            thread::sleep(config.irc.delay.to_std().unwrap());
                        }
                    }
                }
            }
        }
        /*
         * !latest
         */
        else if msg_str.starts_with("!latest") {
            println!("NOT IMPLEMENTED: !latest");
        }

        // All commands below requires OP
        if !config.irc.ops.contains(&msg_source) {
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
    news_list: Arc<Mutex<VecDeque<News>>>,
) {
    for event in irc_reader.iter() {
        if gruik_config.irc.debug {
            dbg!(&event);
        }
        match event {
            loirc::Event::Message(msg) => {
                handle_irc_messages(gruik_config, irc_writer, msg, &news_list);
            }
            _ => {
                println!("Don't know what to do with the following event :");
                dbg!(event);
            }
        }
    }
}

fn mk_hash(links: &Vec<String>) -> String {
    base16ct::lower::encode_string(&Sha256::digest(links.join("")))[..8].to_string()
}

fn news_exists(news: &News, news_list: &Arc<Mutex<VecDeque<News>>>) -> bool {
    for n in news_list.lock().unwrap().iter() {
        if n.hash == news.hash {
            return true;
        }
    }
    false
}

fn fmt_news(news: &News) -> String {
    format!(
        "[{}{}{}] {}{}{} {}{}{} {}#{}{}",
        "\x0313",
        news.origin,
        "\x0f",
        "\x02",
        news.title,
        "\x0f",
        "\x0312",
        news.links.get(0).unwrap(),
        "\x0f",
        "\x0315",
        news.hash,
        "\x0f"
    )
    .to_string()
}

/*
 * This function runs in its own thread
 *
 * Fetch and post news from RSS feeds
 */
fn news_fetch(
    config: Arc<GruikConfig>,
    news_list: Arc<Mutex<VecDeque<News>>>,
    irc_writer: loirc::Writer,
) {
    let feed_file = config.irc.channel.to_owned() + "-feed.json";

    // load saved news
    let mut f = match fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(&feed_file)
    {
        Ok(r) => r,
        Err(e) => {
            println!("Can't open {} : {}", feed_file, e);
            std::process::exit(1);
        }
    };

    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap_or(0);
    *news_list.lock().unwrap() = serde_json::from_str(&buf).unwrap_or(VecDeque::new());

    loop {
        for feed_url in &config.feeds.urls {
            println!("Fetching {feed_url}");
            let response = ureq::get(&feed_url).call();
            if response.is_ok() {
                let body = response.unwrap().into_string();
                if body.is_ok() {
                    let feed = feed_rs::parser::parse(body.unwrap().as_bytes());
                    if feed.is_ok() {
                        let feed = feed.unwrap();
                        let mut i = 0;
                        for item in feed.entries {
                            let origin = match feed.title {
                                Some(ref r) => r.content.to_owned(),
                                None => "Unknown".to_string(),
                            };
                            let date = match item.published {
                                Some(r) => r,
                                None => Utc::now(),
                            };
                            let title = match item.title {
                                Some(r) => r.content,
                                None => "Unknown".to_string(),
                            };
                            let mut links = vec![];
                            for link in item.links {
                                links.push(link.href);
                            }
                            let news = News {
                                origin: origin,
                                date: date,
                                title: title,
                                hash: mk_hash(&links),
                                links: links,
                            };
                            // Check if item was already posted
                            if news_exists(&news, &news_list) {
                                println!("already posted {} ({})", news.title, news.hash);
                                continue;
                            }
                            // don't paste news older than feeds.maxage
                            if Utc::now() - news.date > config.feeds.maxage {
                                println!("news too old {}", news.date);
                                continue;
                            }
                            i = i + 1;
                            if i > config.feeds.maxnews {
                                println!("too many lines to post");
                                break;
                            }

                            if let Err(e) = irc_writer.raw(format!(
                                "PRIVMSG {} {}\n",
                                &config.irc.channel,
                                fmt_news(&news)
                            )) {
                                println!("Failed to send an IRC message... ({:?})", e);
                            }
                            thread::sleep(config.irc.delay.to_std().unwrap());

                            // Mark item as posted
                            {
                                let mut news_list_guarded = news_list.lock().unwrap();

                                news_list_guarded.push_back(news);
                                if news_list_guarded.len() > config.feeds.ringsize {
                                    news_list_guarded.pop_front();
                                }
                            }
                        }
                    } else {
                        println!("Failed to parse feed : {:?}", feed.err());
                    }
                } else {
                    println!("Failed to got body : {:?}", body.err());
                }
            } else {
                println!("Failed to get a response : {:?}", response.err());
            }
        }

        // save news list to disk to avoid repost when restarting
        match f.set_len(0) {
            Ok(_) => {
                if let Err(e) = f.write_all(
                    serde_json::to_string(&*news_list.lock().unwrap())
                        .unwrap_or("".to_string())
                        .as_bytes(),
                ) {
                    println!("Failed to write {} : {}", feed_file, e);
                }
            }
            Err(e) => {
                println!("Failed to truncate {} : {}", feed_file, e);
            }
        }

        thread::sleep(config.feeds.frequency.to_std().unwrap());
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
            println!("Can't read '{config_filename}' : {e}\nexiting.");
            std::process::exit(1);
        }
    };

    let gruik_config: Arc<GruikConfig> = match serde_yaml::from_str(&yaml) {
        Ok(r) => Arc::new(r),
        Err(e) => {
            println!("Can't parse '{config_filename}' : {e}\nexiting.");
            std::process::exit(1);
        }
    };

    let (irc_writer, irc_reader) = match loirc::connect(
        format!("{}:{}", gruik_config.irc.server, gruik_config.irc.port),
        loirc::ReconnectionSettings::Reconnect {
            max_attempts: 10,
            delay_between_attempts: std::time::Duration::from_secs(2),
            delay_after_disconnect: std::time::Duration::from_secs(2),
        },
        encoding::all::UTF_8,
    ) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't connect to IRC server : {e}\nexiting.");
            std::process::exit(1);
        }
    };

    // register
    if let Err(e) = irc_writer.raw(format!("NICK {}\n", &gruik_config.irc.nick)) {
        println!("Can't send the 'NICK' command : {:?}\nexiting.", e);
        std::process::exit(1);
    }

    if let Err(e) = irc_writer.raw(format!(
        "USER {} 0 * :{}\n",
        &gruik_config.irc.nick, &gruik_config.irc.nick
    )) {
        println!("Can't send the 'USER' command : {:?}\nexiting.", e);
        std::process::exit(1);
    }

    let gruik_config_clone = gruik_config.clone();
    let news_list: Arc<Mutex<VecDeque<News>>> = Arc::new(Mutex::new(VecDeque::new()));
    let news_list_clone = news_list.clone();
    let irc_writer_clone = irc_writer.clone();
    thread::spawn(|| news_fetch(gruik_config_clone, news_list_clone, irc_writer_clone));

    // *Warning*, this is a *blocking* function!
    handle_irc_events(&gruik_config, &irc_writer, &irc_reader, news_list);
}
