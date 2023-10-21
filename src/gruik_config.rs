use duration_string::DurationString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, fs, sync::Arc, sync::Mutex};

/*
 * Color codes from :
 * https://modern.ircdocs.horse/formatting#colors
 * https://github.com/lrstanley/girc/blob/master/format.go#L27
 */
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum IrcColor {
    Bold,        // 0x02
    Reset,       // 0x0f
    Italic,      // 0x1d
    Underline,   // 0x1f
    White,       // 00
    Black,       // 01
    Blue,        // 02
    Navy,        // 02
    Green,       // 03
    Red,         // 04
    Brown,       // 05
    Maroon,      // 05
    Magenta,     // 06
    Purple,      // 06
    Orange,      // 07
    Gold,        // 07
    Olive,       // 07
    Yellow,      // 08
    LightGreen,  // 09
    Lime,        // 09
    Cyan,        // 10
    Teal,        // 10
    LightCyan,   // 11
    LightBlue,   // 12
    Royal,       // 12
    Pink,        // 13
    Fuchsia,     // 13
    LightPurple, // 13
    Grey,        // 14
    Gray,        // 14
    LightGrey,   // 15
    Silver,      // 15
}

#[rustfmt::skip]
impl fmt::Display for IrcColor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Self::Bold        => "\x02",
            Self::Reset       => "\x0f",
            Self::Italic      => "\x1d",
            Self::Underline   => "\x1f",
            Self::White       => "\x0300",
            Self::Black       => "\x0301",
            Self::Blue
          | Self::Navy        => "\x0302",
            Self::Green       => "\x0303",
            Self::Red         => "\x0304",
            Self::Brown
          | Self::Maroon      => "\x0305",
            Self::Magenta
          | Self::Purple      => "\x0306",
            Self::Orange
          | Self::Gold
          | Self::Olive       => "\x0307",
            Self::Yellow      => "\x0308",
            Self::LightGreen
          | Self::Lime        => "\x0309",
            Self::Cyan
          | Self::Teal        => "\x0310",
            Self::LightCyan   => "\x0311",
            Self::LightBlue
          | Self::Royal       => "\x0312",
            Self::Pink
          | Self::Fuchsia
          | Self::LightPurple => "\x0313",
            Self::Grey
          | Self::Gray        => "\x0314",
            Self::LightGrey
          | Self::Silver      => "\x0315",
        };
        write!(f, "{printable}")
    }
}

#[rustfmt::skip]
impl<'de> Deserialize<'de> for IrcColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "bold"        => Ok(Self::Bold),
            "italic"      => Ok(Self::Italic),
            "underline"   => Ok(Self::Underline),
            "white"       => Ok(Self::White),
            "black"       => Ok(Self::Black),
            "blue"        => Ok(Self::Blue),
            "navy"        => Ok(Self::Navy),
            "green"       => Ok(Self::Green),
            "red"         => Ok(Self::Red),
            "brown"       => Ok(Self::Brown),
            "maroon"      => Ok(Self::Maroon),
            "magenta"     => Ok(Self::Magenta),
            "purple"      => Ok(Self::Purple),
            "orange"      => Ok(Self::Orange),
            "gold"        => Ok(Self::Gold),
            "olive"       => Ok(Self::Olive),
            "yellow"      => Ok(Self::Yellow),
            "lightgreen"  => Ok(Self::LightGreen),
            "lime"        => Ok(Self::Lime),
            "cyan"        => Ok(Self::Cyan),
            "teal"        => Ok(Self::Teal),
            "lightcyan"   => Ok(Self::LightCyan),
            "lightblue"   => Ok(Self::LightBlue),
            "royal"       => Ok(Self::Royal),
            "pink"        => Ok(Self::Pink),
            "fuchsia"     => Ok(Self::Fuchsia),
            "lightpurple" => Ok(Self::LightPurple),
            "grey"        => Ok(Self::Grey),
            "gray"        => Ok(Self::Gray),
            "lightgrey"   => Ok(Self::LightGrey),
            "silver"      => Ok(Self::Silver),
            other   => Err(format!("Unknown color '{other}'")).map_err(D::Error::custom),
        }
    }
}

#[rustfmt::skip]
impl Serialize for IrcColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {

        serializer.serialize_str(match *self {
            Self::Bold        => "bold",
            Self::Italic      => "italic",
            Self::Underline   => "underline",
            Self::White       => "white",
            Self::Black       => "black",
            Self::Blue        => "blue",
            Self::Navy        => "navy",
            Self::Green       => "green",
            Self::Red         => "red",
            Self::Brown       => "brown",
            Self::Maroon      => "maroon",
            Self::Magenta     => "magenta",
            Self::Purple      => "purple",
            Self::Orange      => "orange",
            Self::Gold        => "gold",
            Self::Olive       => "olive",
            Self::Yellow      => "yellow",
            Self::LightGreen  => "lightgreen",
            Self::Lime        => "lime",
            Self::Cyan        => "cyan",
            Self::Teal        => "teal",
            Self::LightCyan   => "lightcyan",
            Self::LightBlue   => "lightblue",
            Self::Royal       => "royal",
            Self::Pink        => "pink",
            Self::Fuchsia     => "fuchsia",
            Self::LightPurple => "lightpurple",
            Self::Grey        => "grey",
            Self::Gray        => "gray",
            Self::LightGrey   => "lightgrey",
            Self::Silver      => "silver",
            Self::Reset       => "reset", // This is just here to please the rust compiler
                                             // because the deserializer won't allow this value
        })
    }
}

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
    colors: HashMap<String, IrcColor>,
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
                ("origin".to_string(), IrcColor::Pink),
                ("title".to_string(), IrcColor::Bold),
                ("hash".to_string(), IrcColor::LightGrey),
                ("link".to_string(), IrcColor::LightBlue),
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
    pub fn origin_color(&self) -> IrcColor {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .colors
            .get("origin")
            .unwrap_or(&IrcColor::Pink)
            .clone()
    }
    pub fn title_color(&self) -> IrcColor {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .colors
            .get("title")
            .unwrap_or(&IrcColor::Bold)
            .clone()
    }
    pub fn hash_color(&self) -> IrcColor {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .colors
            .get("hash")
            .unwrap_or(&IrcColor::LightGrey)
            .clone()
    }
    pub fn link_color(&self) -> IrcColor {
        self.inner
            .lock()
            .expect("Poisoned lock!")
            .irc
            .colors
            .get("link")
            .unwrap_or(&IrcColor::LightBlue)
            .clone()
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
                    Ok(()) => {}
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
                    Ok(()) => Ok(()),
                    Err(e) => Err(format!("rmfeed(): failed to write config file: {e}")),
                }
            }
            Err(e) => Err(format!(
                "rmfeed(): failed to serialize GruikConfigYaml: {e}"
            )),
        }
    }
}
