mod gruik_config;

use chrono::{DateTime, Utc};
use gruik_config::GruikConfig;
use loirc::Message;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::{env, fs, sync::Arc, sync::Mutex, thread};
use tokio::task::JoinSet;

use crate::gruik_config::IrcColor;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
struct News {
    origin: String,
    title: String,
    links: Vec<String>,
    date: DateTime<Utc>,
    hash: String,
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
            Ok(()) => {
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
                res.push(
                    news_list_guarded
                        .get(len - i)
                        .expect("Missing news in slice ?!?")
                        .clone(),
                );
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
    use loirc::Prefix::{Server, User};

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
                            fmt_news(gruik_config, &news),
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

            let origin: &[&str] = msg_args.get(1..).map_or(&[], |v| v);

            for news in news_list.get_latest(n, origin) {
                if let Err(e) = irc_writer.raw(format!(
                    "PRIVMSG {} {}\n",
                    msg_source,
                    fmt_news(gruik_config, &news)
                )) {
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
                Ok(()) => "feed removed".to_string(),
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
    use sha2::{Digest, Sha256};
    base16ct::lower::encode_string(&Sha256::digest(links.join("")))[..8].to_string()
}

fn fmt_news(gruik_config: &GruikConfig, news: &News) -> String {
    format!(
        "[{}{}{}] {}{}{} {}{}{} {}#{}{}",
        gruik_config.origin_color(),
        news.origin,
        IrcColor::Reset,
        gruik_config.title_color(),
        news.title,
        IrcColor::Reset,
        gruik_config.link_color(),
        news.links
            .first()
            .expect("At least one link should be present!"),
        IrcColor::Reset,
        gruik_config.hash_color(),
        news.hash,
        IrcColor::Reset
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
            let response = match ureq::get(feed_url.as_str()).call() {
                Ok(r) => r,
                Err(e) => {
                    println!("Failed to get a response : {e:?}");
                    continue;
                }
            };

            let mut body = response.into_body();

            let feed = match feed_rs::parser::parse(body.as_reader()) {
                Ok(r) => r,
                Err(e) => {
                    println!("Failed to parse feed : {e:?}");
                    continue;
                }
            };

            let mut i = 0;
            for item in feed.entries {
                let origin = feed
                    .title
                    .as_ref()
                    .map_or_else(|| "Unknown".to_string(), |s| s.content.clone());
                let date = item.published.map_or_else(Utc::now, |s| s);
                let title = item.title.map_or("Unknown".to_string(), |v| v.content);
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
                    fmt_news(gruik_config, &news)
                )) {
                    println!("Failed to send an IRC message... ({e:?})");
                }
                thread::sleep(gruik_config.irc_delay());

                // Mark item as posted
                news_list.add(news, gruik_config.feeds_ringsize());
            }
        }

        // save news list to disk to avoid repost when restarting
        news_list.save_file(&feed_file);

        thread::sleep(gruik_config.feeds_frequency());
    }
}

fn config_filename_notify(gruik_config: &GruikConfig) {
    use notify::{
        Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind,
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher =
        RecommendedWatcher::new(tx, Config::default()).expect("Couldn't set FS event watcher");
    watcher
        .watch(
            std::path::Path::new(&gruik_config.filename),
            RecursiveMode::NonRecursive,
        )
        .expect("Couldn't set FS event watch on config_filename");

    for res in rx {
        match res {
            Ok(event) => {
                if let EventKind::Modify(ModifyKind::Data(_)) = event.kind {
                    gruik_config.reload();
                }
            }
            Err(error) => println!("Error: {error:?}"),
        }
    }
}

#[tokio::main]
async fn main() {
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

    /*
     * From here, we are going to create 3 blocking tasks :
     *
     * #1 will run news_fetch()
     * #2 will run config_filename_notify()
     * #3 will run handle_irc_events()
     *
     * As soon as one of the tasks finishes, the whole program will exit!!!
     */

    let gruik_config_clone1 = gruik_config.clone();
    let gruik_config_clone2 = gruik_config.clone();
    let news_list = NewsList::new();
    let news_list_clone1 = news_list.clone();
    let irc_writer_clone1 = irc_writer.clone();

    let mut set = JoinSet::new();

    set.spawn_blocking(move || {
        news_fetch(&gruik_config_clone1, &news_list_clone1, &irc_writer_clone1);
    });

    set.spawn_blocking(move || config_filename_notify(&gruik_config_clone2));

    set.spawn_blocking(move || {
        handle_irc_events(&gruik_config, &irc_writer, &irc_reader, &news_list);
    });

    // We wait for one of the blocking tasks to exit
    set.join_next().await;
    println!("now exiting because one the tasks finished");
    std::process::exit(0);
}
