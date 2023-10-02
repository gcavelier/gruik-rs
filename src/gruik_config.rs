use duration_string::DurationString;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, fs, sync::Arc, sync::Mutex};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
struct IrcConfig {
    server: String,
    nick: String,
    channel: String,
    xchannels: Vec<String>,
    password: Option<String>,
    debug: bool,
    port: u16,
    delay: DurationString,
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
            delay: DurationString::from_str("2s").expect("Wrong default!"),
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
struct FeedsConfig {
    urls: Vec<String>,
    maxnews: u16,
    maxage: DurationString,
    frequency: DurationString,
    ringsize: usize,
}

impl Default for FeedsConfig {
    fn default() -> Self {
        Self {
            urls: vec![],
            maxnews: 10,
            maxage: DurationString::from_str("1h").expect("Wrong default!"),
            frequency: DurationString::from_str("30m").expect("Wrong default!"),
            ringsize: 100,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GruikConfigYaml {
    irc: IrcConfig,
    feeds: FeedsConfig,
}

// The following structure allows sharing the config between multiple threads (or coroutines)
// It "masks" the internal structure (and the mutex) and you should use the implementations to
// get/set values
pub struct GruikConfig {
    inner: Arc<Mutex<GruikConfigYaml>>,
    pub filename: String,
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
    pub fn new(filename: String) -> Self {
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
    pub fn reload(&self) {
        // TODO : Code is duplicated from the new() function above
        let yaml = match fs::read_to_string(&self.filename) {
            Ok(r) => r,
            Err(e) => {
                println!("Can't read '{}' : {e}\nexiting.", &self.filename);
                std::process::exit(1);
            }
        };

        let gruik_config_yaml: GruikConfigYaml = match serde_yaml::from_str(&yaml) {
            Ok(r) => r,
            Err(e) => {
                println!("Can't parse '{}' : {e}\nexiting.", &self.filename);
                std::process::exit(1);
            }
        };
        *self.inner.lock().expect("Poisoned lock!") = gruik_config_yaml;
    }
    pub fn irc_server(&self) -> String {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .server
            .clone()
    }
    pub fn irc_port(&self) -> u16 {
        self.inner.lock().expect("Poisoned lock!").irc.port
    }
    pub fn irc_nick(&self) -> String {
        self.inner.lock().expect("Poisoned lock!").irc.nick.clone()
    }
    pub fn irc_channel(&self) -> String {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .channel
            .clone()
    }
    pub fn xchannels(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().expect("Poisoned lock!").irc.xchannels {
            vec.push(channel.clone());
        }
        vec
    }
    pub fn feeds_urls(&self) -> Vec<String> {
        let mut vec = Vec::new();
        for channel in &self.inner.lock().expect("Poisoned lock!").feeds.urls {
            vec.push(channel.clone());
        }
        vec
    }
    pub fn irc_delay(&self) -> Duration {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .delay
            .try_into()
            .map_or_else(|_| Duration::new(2, 0), |d| d)
    }
    pub fn is_ops(&self, user: &String) -> bool {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .ops
            .contains(user)
    }
    pub fn debug(&self) -> bool {
        self.inner.lock().expect("Poisoned lock!").irc.debug
    }
    pub fn feeds_maxage(&self) -> chrono::Duration {
        let std_duration: Duration = self
            .inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .maxage
            .into();
        chrono::Duration::from_std(std_duration).expect("Wrong conversion!")
    }
    pub fn feeds_frequency(&self) -> Duration {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .frequency
            .try_into()
            .map_or_else(|_| Duration::new(10 * 60, 0), |d| d)
    }
    pub fn feeds_maxnews(&self) -> u16 {
        self.inner.lock().expect("Poisoned lock!").feeds.maxnews
    }
    pub fn feeds_ringsize(&self) -> usize {
        self.inner.lock().expect("Poisoned lock!").feeds.ringsize
    }
    pub fn addfeed(&self, url: String) {
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
        // We rewrite the config file with the new feed
        match serde_yaml::to_string(&*self.inner.lock().expect("Poisoned lock")) {
            Ok(s) => {
                // Serialization is ok, writing the result to a file
                match fs::write(&self.filename, s) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("addfeed(): Failed to write the new config filename: {e}");
                    }
                }
            }
            Err(e) => println!("addfeed(): Failed to serialize GruikConfigYaml: {e}"),
        }
    }
    pub fn rmfeed(&self, index: usize) -> Result<(), String> {
        if index > self.inner.lock().expect("Poisoned lock!").feeds.urls.len() {
            return Err("bad index number".to_string());
        }
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .feeds
            .urls
            .remove(index);
        // We rewrite the config file
        match serde_yaml::to_string(&*self.inner.lock().expect("Poisoned lock")) {
            Ok(s) => {
                // Serialization is ok, writing the result to a file
                match fs::write(&self.filename, s) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("rmfeed(): failed to write config file: {e}")),
                }
            }
            Err(e) => Err(format!(
                "rmfeed(): failed to serialize GruikConfigYaml: {e}"
            )),
        }
    }
}
