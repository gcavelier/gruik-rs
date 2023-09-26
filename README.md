This project is a rewrite of [gruik](https://gitlab.com/gcu-squad/gruik/) in rust

I started it to learn rust.

To be complete, the following features are needed :

- [X] Get an optionnal argument
- [X] Read a yaml config file
- [X] Hot reloading of the configuration when the yaml file is changed
- [X] Rewrite the configuration file when a feed in added or removed
- [X] Connect to an IRC server
- [X] Join configured xchannels
- [X] Handle private messages
- [X] Load news list from a JSON file
- [X] Write news list to a JSON file
- [X] Fetch and parse RSS feeds
- [X] Post RSS news
- [X] Handle IRC disconnects

# Enhancements to implement

- [ ] Deserialize colors (IrcConfig.colors.{origin,title,hash,link}) to an enum to check if their values are ok
- [ ] Reduce the use of unwrap()
- [ ] Use an async runtime instead of threads
- [ ] Better error handling

# Notes

To start a local IRC server :
```sh
docker run --rm --name inspircd -p 6667:6667 -e "INSP_ENABLE_DNSBL=no" -e "INSP_SERVER_NAME=irc.example.com" inspircd/inspircd-docker --debug
```

# IRC Numerics
https://modern.ircdocs.horse/#numerics

# Clippy parameters
```sh
cargo clippy -- -W clippy::pedantic -W clippy::nursery -W clippy::unwrap_used
```
