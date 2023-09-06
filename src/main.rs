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

#[derive(Debug, Deserialize, Serialize, Clone)]
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
struct GruikConfig {
    inner: Arc<Mutex<GruikConfigYaml>>,
    filename: String,
}

impl Clone for GruikConfig {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            filename: self.filename.clone(),
        }
    }
}

impl GruikConfig {
    fn new(filename: String) -> Self {
        let yaml = match fs::read_to_string(&filename) {
            Ok(r) => r,
            Err(e) => {
                println!("Can't read '{}' : {e}\nexiting.", &filename);
                std::process::exit(1);
            }
        };

        let gruik_config_yaml: GruikConfigYaml = match serde_yaml::from_str(&yaml) {
            Ok(r) => r,
            Err(e) => {
                println!("Can't parse '{}' : {e}\nexiting.", &filename);
                std::process::exit(1);
            }
        };
        Self {
            inner: Arc::new(Mutex::new(gruik_config_yaml)),
            filename,
        }
    }
    fn irc_server(&self) -> String {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .server
            .clone()
    }
    fn irc_port(&self) -> u16 {
        self.inner.lock().expect("Poisoned lock!").irc.port
    }
    fn irc_nick(&self) -> String {
        self.inner.lock().expect("Poisoned lock!").irc.nick.clone()
    }
    fn irc_channel(&self) -> String {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .channel
            .clone()
    }
    fn xchannels(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().expect("Poisoned lock!").irc.xchannels {
            vec.push(channel.clone());
        }
        vec
    }
    fn feeds_urls(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().expect("Poisoned lock!").feeds.urls {
            vec.push(channel.clone());
        }
        vec
    }
    fn irc_delay(&self) -> std::time::Duration {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .delay
            .to_std()
            .map_or_else(|_| std::time::Duration::new(2, 0), |d| d)
    }
    fn is_ops(&self, user: &String) -> bool {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .ops
            .contains(user)
    }
    fn debug(&self) -> bool {
        self.inner.lock().expect("Poisoned lock!").irc.debug
    }
    fn feeds_maxage(&self) -> Duration {
        self.inner.lock().expect("Poisoned lock!").feeds.maxage
    }
    fn feeds_frequency(&self) -> std::time::Duration {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .frequency
            .to_std()
            .map_or_else(|_| std::time::Duration::new(10 * 60, 0), |d| d)
    }
    fn feeds_maxnews(&self) -> u16 {
        self.inner.lock().expect("Poisoned lock!").feeds.maxnews
    }
    fn feeds_ringsize(&self) -> usize {
        self.inner.lock().expect("Poisoned lock!").feeds.ringsize
    }
    fn addfeed(&self, url: String) {
        if self
            .inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .urls
            .contains(&url)
        {
            return;
        }
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .urls
            .push(url);
    }
    fn rmfeed(&self, index: usize) -> Result<(), String> {
        if index > self.inner.lock().expect("Poisoned lock!").feeds.urls.len() {
            Err("bad index number".to_string())
        } else {
            self.inner
                .lock()
                .expect("Poisoned lock!")
                .feeds
                .urls
                .remove(index);
            Ok(())
        }
    }
}

#[derive(Clone)]
struct NewsList {
    inner: Arc<Mutex<VecDeque<News>>>,
}

impl NewsList {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn contains(&self, news: &News) -> bool {
        for n in &*self.inner.lock().expect("Poisoned lock!") {
            if n.hash == news.hash {
                return true;
            }
        }
        false
    }

    fn get_all(&self) -> VecDeque<News> {
        // We return a copy of the data in the struct
        self.inner.lock().expect("Poisoned lock!").clone()
    }

    fn load_file(&self, feed_file: &String) {
        let mut f = match fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(feed_file)
        {
            Ok(r) => r,
            Err(e) => {
                println!("Can't open {feed_file} : {e}");
                std::process::exit(1);
            }
        };
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap_or(0);
        *self.inner.lock().expect("Poisoned lock!") =
            serde_json::from_str(&buf).unwrap_or_default();
    }

    fn save_file(&self, feed_file: &String) {
        let mut f = match fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(feed_file)
        {
            Ok(r) => r,
            Err(e) => {
                println!("Can't open {feed_file} : {e}");
                std::process::exit(1);
            }
        };
        match f.set_len(0) {
            Ok(_) => {
                if let Err(e) = f.write_all(
                    serde_json::to_string(&*self.inner.lock().expect("Poisoned lock!"))
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
    }
    fn add(&self, news: News, ringsize: usize) {
        let mut news_list_guarded = self.inner.lock().expect("Poisoned lock!");

        if news_list_guarded.len() > ringsize {
            news_list_guarded.pop_front();
        } else {
            news_list_guarded.push_back(news);
        }
    }

    fn get_latest(&self, n: usize, origin: &[&str]) -> Vec<News> {
        let mut res = Vec::new();
        let mut n = n;
        let news_list_guarded = self.inner.lock().expect("Poisoned lock!");
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
                res.push(
                    news_list_guarded
                        .get(len - i)
                        .expect("Missing news in slice ?!?")
                        .clone(),
                );
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
                res.push(news_list_guarded.get(len - i).unwrap().clone());
            }
        }
        res
    }
}

fn handle_irc_messages(
    gruik_config: &GruikConfig,
    irc_writer: &loirc::Writer,
    msg: Message,
    news_list: &NewsList,
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
        let msg_source = msg.prefix.map_or_else(String::new, |s| match s {
            User(u) => u.nickname,
            Server(s) => s,
        });
        let msg_str = msg.args.get(1).unwrap_or(&empty_str);
        let msg_args: Vec<&str> = msg_str.split(' ').collect();
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
            for news in news_list.get_all() {
                println!("{}", news.hash);
                if news.hash == hash {
                    for channel in &xchannels {
                        if let Err(e) = irc_writer.raw(format!(
                            "PRIVMSG {} {} (from {msg_source} on {irc_channel})\n",
                            &channel,
                            fmt_news(&news),
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
            let n = match msg_args.first() {
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

            for news in news_list.get_latest(n, origin) {
                if let Err(e) =
                    irc_writer.raw(format!("PRIVMSG {} {}\n", msg_source, fmt_news(&news)))
                {
                    println!("Failed to send an IRC message... ({e:?})");
                } else {
                    thread::sleep(gruik_config.irc_delay());
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
            let url = match msg_args.first() {
                Some(url) => (*url).to_string(),
                None => return,
            };

            gruik_config.addfeed(url);

            // TODO : use color in the following message
            if let Err(e) = irc_writer.raw(format!("PRIVMSG {msg_source} feed added\n")) {
                println!("Failed to send an IRC message... ({e:?})");
            }
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
                        "PRIVMSG {msg_source} index conversion failed ({e})\n"
                    )) {
                        println!("Failed to send an IRC message... ({e:?})");
                    }
                    return;
                }
            };
            let msg = match gruik_config.rmfeed(index) {
                Ok(_) => "feed removed".to_string(),
                Err(e) => e,
            };

            // TODO : use color in the following message
            if let Err(e) = irc_writer.raw(format!("PRIVMSG {msg_source} {msg}\n")) {
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
    news_list: &NewsList,
) {
    for event in irc_reader {
        if gruik_config.debug() {
            dbg!(&event);
        }
        if let loirc::Event::Message(msg) = event {
            handle_irc_messages(gruik_config, irc_writer, msg, news_list);
        } else {
            println!("Don't know what to do with the following event :");
            dbg!(event);
        }
    }
}

fn mk_hash(links: &[String]) -> String {
    base16ct::lower::encode_string(&Sha256::digest(links.join("")))[..8].to_string()
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
        news.links
            .first()
            .expect("At least one link should be present!"),
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
fn news_fetch(gruik_config: &GruikConfig, news_list: &NewsList, irc_writer: &loirc::Writer) {
    let feed_file = gruik_config.irc_channel() + "-feed.json";

    // load saved news
    news_list.load_file(&feed_file);

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
                            if news_list.contains(&news) {
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
                            news_list.add(news, gruik_config.feeds_ringsize());
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
        news_list.save_file(&feed_file);

        thread::sleep(gruik_config.feeds_frequency());
    }
}
fn main() {
    let args: Vec<String> = env::args().collect();

    let config_filename = args.get(1).map_or("config.yaml", |s| s).to_string();

    // We are now creating a GruikConfig structure so that it can be shared later
    let gruik_config = GruikConfig::new(config_filename);

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
    let news_list = NewsList::new();
    let news_list_clone = news_list.clone();
    let irc_writer_clone = irc_writer.clone();
    thread::spawn(move || news_fetch(&gruik_config_clone, &news_list_clone, &irc_writer_clone));

    // *Warning*, this is a *blocking* function!
    handle_irc_events(&gruik_config, &irc_writer, &irc_reader, &news_list);
}
