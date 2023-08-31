use chrono::{DateTime, Duration, Utc};
use duration_str::deserialize_duration_chrono;
use loirc::Message;
use loirc::Prefix::{Server, User};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::{collections::HashMap, env, fs, sync::Arc, sync::Mutex, thread};

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
        Self {
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
        Self {
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
struct GruikConfigYaml {
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

// The following structure allows sharing the config between multiple threads (or coroutines)
// It "masks" the internal structure (and the mutex) and you should use the implementations to
// get/set values
#[derive(Clone)]
struct GruikConfig {
    inner: Arc<Mutex<GruikConfigYaml>>,
}

impl GruikConfig {
    fn new(config: GruikConfigYaml) -> Self {
        Self {
            inner: Arc::new(Mutex::new(config)),
        }
    }
    fn irc_server(&self) -> String {
        self.inner.lock().unwrap().irc.server.clone()
    }
    fn irc_port(&self) -> u16 {
        self.inner.lock().unwrap().irc.port.clone()
    }
    fn irc_nick(&self) -> String {
        self.inner.lock().unwrap().irc.nick.clone()
    }
    fn irc_channel(&self) -> String {
        self.inner.lock().unwrap().irc.channel.clone()
    }
    fn xchannels(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().unwrap().irc.xchannels {
            vec.push(channel.clone());
        }
        vec
    }
    fn feeds_urls(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().unwrap().feeds.urls {
            vec.push(channel.clone());
        }
        vec
    }
    fn irc_delay(&self) -> std::time::Duration {
        self.inner.lock().unwrap().irc.delay.to_std().unwrap()
    }
    fn is_ops(&self, user: &String) -> bool {
        self.inner.lock().unwrap().irc.ops.contains(user)
    }
    fn debug(&self) -> bool {
        self.inner.lock().unwrap().irc.debug
    }
    fn feeds_maxage(&self) -> Duration {
        self.inner.lock().unwrap().feeds.maxage
    }
    fn feeds_frequency(&self) -> std::time::Duration {
        self.inner.lock().unwrap().feeds.frequency.to_std().unwrap()
    }
    fn feeds_maxnews(&self) -> u16 {
        self.inner.lock().unwrap().feeds.maxnews
    }
    fn feeds_ringsize(&self) -> usize {
        self.inner.lock().unwrap().feeds.ringsize
    }
}

fn handle_irc_messages(
    gruik_config: &GruikConfig,
    irc_writer: &loirc::Writer,
    msg: Message,
    news_list: &Arc<Mutex<VecDeque<News>>>,
) {
    let irc_channel = gruik_config.irc_channel();
    let xchannels = gruik_config.xchannels();

    /*
     * PING
     */
    if msg.code == loirc::Code::Ping {
        let ping_arg = msg.args.get(0).map_or_else(
            || {
                println!("Can't get ping argument! exiting.");
                std::process::exit(1);
            },
            |s| s,
        );
        if let Err(e) = irc_writer.raw(format!("PONG :{ping_arg}\n")) {
            println!("Couldn't send the 'PONG' command{e:?}");
        }
        return;
    }
    /*
     * RPL_WELCOME
     */
    if msg.code == loirc::Code::RplWelcome {
        if let Err(e) = irc_writer.raw(format!("JOIN {irc_channel}\n")) {
            println!("Couldn't join {irc_channel} : {e:?}");
        }
        for channel in xchannels {
            if let Err(e) = irc_writer.raw(format!("JOIN {channel}\n")) {
                println!("Couldn't join {channel} : {e:?}");
            }
        }
        return;
    }
    /*
     * PRIVMSG
     */
    if msg.code == loirc::Code::Privmsg {
        let empty_str = String::new();
        let msg_source = msg.prefix.map_or_else(
            || String::new(),
            |s| match s {
                User(u) => u.nickname,
                Server(s) => s,
            },
        );
        let msg_str = msg.args.get(1).unwrap_or(&empty_str);
        let msg_args: Vec<&str> = msg_str.split(" ").collect();
        let (_, msg_args) = msg_args.split_at(1);

        /*
         * !lsfeeds
         */
        if msg_str.starts_with("!lsfeeds") {
            for (i, feed) in gruik_config.feeds_urls().iter().enumerate() {
                if let Err(e) = irc_writer.raw(format!("PRIVMSG {} {}. {feed}\n", &msg_source, i)) {
                    println!("Failed to send an IRC message... ({e:?})");
                } else {
                    thread::sleep(gruik_config.irc_delay());
                }
            }
        }
        /*
         * !xpost
         */
        else if msg_str.starts_with("!xpost") {
            let hash = msg_args
                .first()
                .map_or_else(String::new, |s| s.replace('#', ""));
            for news in news_list.lock().unwrap().iter() {
                println!("{}", news.hash);
                if news.hash == hash {
                    for channel in &xchannels {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {} (from {msg_source} on {irc_channel})\n",
                            &channel,
                            fmt_news(news),
                        )) {
                            println!("Failed to send an IRC message... ({e:?})");
                        } else {
                            thread::sleep(gruik_config.irc_delay());
                        }
                    }
                }
            }
        }
        /*
         * !latest
         */
        else if msg_str.starts_with("!latest") {
            if msg_args.is_empty() {
                if let Err(e) = irc_writer.raw(format!(
                    "PRIVMSG {} {}\n",
                    msg_source, "usage: !latest <number> [origin]"
                )) {
                    println!("Failed to send an IRC message... ({e:?})");
                } else {
                    thread::sleep(gruik_config.irc_delay());
                }
                return;
            }

            // n == number of news to show
            let mut n = match msg_args.get(0) {
                None => 0,
                Some(arg) => match arg.parse() {
                    Err(_) => {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {}\n",
                            msg_source, "!latest : conversion error"
                        )) {
                            println!("Failed to send an IRC message... ({e:?})");
                        } else {
                            thread::sleep(gruik_config.irc_delay());
                        }
                        return;
                    }
                    Ok(n) => n,
                },
            };

            let origin = msg_args.get(1..).unwrap();

            {
                let news_list_guarded = news_list.lock().unwrap();
                if origin.is_empty() {
                    let len = if news_list_guarded.len() > 1 {
                        news_list_guarded.len() - 1
                    } else {
                        0
                    };
                    if n > len {
                        n = len;
                    }
                    for i in 0..n {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {}\n",
                            msg_source,
                            fmt_news(news_list_guarded.get(len - i).unwrap())
                        )) {
                            println!("Failed to send an IRC message... ({e:?})");
                        } else {
                            thread::sleep(gruik_config.irc_delay());
                        }
                    }
                } else {
                    let origin = origin.join(" ");
                    let show_news: Vec<&News> = news_list_guarded
                        .iter()
                        .filter(|x| *x.origin == origin)
                        .collect();
                    let len = if show_news.len() > 1 {
                        show_news.len() - 1
                    } else {
                        0
                    };
                    if n > len {
                        n = len;
                    }

                    for i in 0..n {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {}\n",
                            msg_source,
                            fmt_news(show_news.get(len - i).unwrap())
                        )) {
                            println!("Failed to send an IRC message... ({e:?})");
                        } else {
                            thread::sleep(gruik_config.irc_delay());
                        }
                    }
                }
            }

            return;
        }

        // All commands below requires OP
        if !gruik_config.is_ops(&msg_source) {
            return;
        }

        /*
         * !die
         */
        if msg_str.starts_with("!die") {
            irc_writer
                .disconnect()
                .expect("Disconnect should not fail!");
            std::process::exit(0);
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
            // This will delete a feed, based on its index
            let index: usize = match msg_args.first().unwrap_or(&"").parse() {
                Ok(r) => r,
                Err(e) => {
                    if let Err(e) = irc_writer.raw(format!(
                        "PRIVMSG {} index conversion failed ({e})\n",
                        msg_source
                    )) {
                        println!("Failed to send an IRC message... ({e:?})");
                    }
                    return;
                }
            };
            if index > gruik_config.inner.lock().unwrap().feeds.urls.len() {
                if let Err(e) = irc_writer.raw(format!("PRIVMSG {} bad index number\n", msg_source))
                {
                    println!("Failed to send an IRC message... ({e:?})",);
                }
                return;
            }
            gruik_config.inner.lock().unwrap().feeds.urls.remove(index);
            // TODO : use color in the following message
            if let Err(e) = irc_writer.raw(format!("PRIVMSG {} feed removed\n", msg_source)) {
                println!("Failed to send an IRC message... ({e:?})");
            }
        }

        // We discard all other messages
    }
}

fn handle_irc_events(
    gruik_config: &GruikConfig,
    irc_writer: &loirc::Writer,
    irc_reader: &loirc::Reader,
    news_list: &Arc<Mutex<VecDeque<News>>>,
) {
    for event in irc_reader.iter() {
        if gruik_config.debug() {
            dbg!(&event);
        }
        match event {
            loirc::Event::Message(msg) => {
                handle_irc_messages(gruik_config, irc_writer, msg, news_list);
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
}

/*
 * This function runs in its own thread
 *
 * Fetch and post news from RSS feeds
 */
fn news_fetch(
    gruik_config: &GruikConfig,
    news_list: &Arc<Mutex<VecDeque<News>>>,
    irc_writer: &loirc::Writer,
) {
    let feed_file = gruik_config.irc_channel().to_owned() + "-feed.json";

    // load saved news
    let mut f = match fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(&feed_file)
    {
        Ok(r) => r,
        Err(e) => {
            println!("Can't open {feed_file} : {e}");
            std::process::exit(1);
        }
    };

    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap_or(0);
    *news_list.lock().unwrap() = serde_json::from_str(&buf).unwrap_or(VecDeque::new());

    loop {
        for feed_url in gruik_config.feeds_urls() {
            println!("Fetching {feed_url}");
            let response = ureq::get(feed_url.as_str()).call();
            if response.is_ok() {
                let body = response.unwrap().into_string();
                if body.is_ok() {
                    let feed = feed_rs::parser::parse(body.unwrap().as_bytes());
                    if feed.is_ok() {
                        let feed = feed.unwrap();
                        let mut i = 0;
                        for item in feed.entries {
                            let origin = feed
                                .title
                                .as_ref()
                                .map_or_else(|| "Unknown".to_string(), |s| s.content.clone());
                            let date = item.published.map_or_else(Utc::now, |s| s);
                            let title = match item.title {
                                Some(r) => r.content,
                                None => "Unknown".to_string(),
                            };
                            let mut links = vec![];
                            for link in item.links {
                                links.push(link.href);
                            }
                            let news = News {
                                origin,
                                date,
                                title,
                                hash: mk_hash(&links),
                                links,
                            };
                            // Check if item was already posted
                            if news_exists(&news, news_list) {
                                println!("already posted {} ({})", news.title, news.hash);
                                continue;
                            }
                            // don't paste news older than feeds.maxage
                            if Utc::now() - news.date > gruik_config.feeds_maxage() {
                                println!("news too old {}", news.date);
                                continue;
                            }
                            i += 1;
                            if i > gruik_config.feeds_maxnews() {
                                println!("too many lines to post");
                                break;
                            }

                            if let Err(e) = irc_writer.raw(format!(
                                "PRIVMSG {} {}\n",
                                &gruik_config.irc_channel(),
                                fmt_news(&news)
                            )) {
                                println!("Failed to send an IRC message... ({e:?})");
                            }
                            thread::sleep(gruik_config.irc_delay());

                            // Mark item as posted
                            {
                                let mut news_list_guarded = news_list.lock().unwrap();

                                if news_list_guarded.len() > gruik_config.feeds_ringsize() {
                                    news_list_guarded.pop_front();
                                } else {
                                    news_list_guarded.push_back(news);
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
                        .unwrap_or_default()
                        .as_bytes(),
                ) {
                    println!("Failed to write {feed_file} : {e}");
                }
            }
            Err(e) => {
                println!("Failed to truncate {feed_file} : {e}");
            }
        }

        thread::sleep(gruik_config.feeds_frequency());
    }
}
fn main() {
    let args: Vec<String> = env::args().collect();

    let config_filename = args.get(1).map_or("config.yaml", |s| s);

    let yaml = match fs::read_to_string(config_filename) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't read '{config_filename}' : {e}\nexiting.");
            std::process::exit(1);
        }
    };

    let gruik_config_yaml: GruikConfigYaml = match serde_yaml::from_str(&yaml) {
        Ok(r) => r,
        Err(e) => {
            println!("Can't parse '{config_filename}' : {e}\nexiting.");
            std::process::exit(1);
        }
    };

    // We are now creating a GruikConfig structure so that it can be shared later
    let gruik_config = GruikConfig::new(gruik_config_yaml);

    let (irc_writer, irc_reader) = match loirc::connect(
        format!("{}:{}", gruik_config.irc_server(), gruik_config.irc_port()),
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
    let irc_nick = gruik_config.irc_nick();
    if let Err(e) = irc_writer.raw(format!("NICK {irc_nick}\n")) {
        println!("Can't send the 'NICK' command : {e:?}\nexiting.");
        std::process::exit(1);
    }

    if let Err(e) = irc_writer.raw(format!("USER {irc_nick} 0 * :{irc_nick}\n")) {
        println!("Can't send the 'USER' command : {e:?}\nexiting.");
        std::process::exit(1);
    }

    let gruik_config_clone = gruik_config.clone();
    let news_list: Arc<Mutex<VecDeque<News>>> = Arc::new(Mutex::new(VecDeque::new()));
    let news_list_clone = news_list.clone();
    let irc_writer_clone = irc_writer.clone();
    thread::spawn(move || news_fetch(&gruik_config_clone, &news_list_clone, &irc_writer_clone));

    // *Warning*, this is a *blocking* function!
    handle_irc_events(&gruik_config, &irc_writer, &irc_reader, &news_list);
}
