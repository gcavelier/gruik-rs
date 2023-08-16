This project is a rewrite of [gruik](https://gitlab.com/gcu-squad/gruik/) in rust

I started it to learn rust.

To be complete, the following features are needed :

- [X] Get an optionnal argument
- [X] Read a yaml config file
- [ ] Hot reloading of the configuration when the yaml file is changed
- [X] Connect to an IRC server
- [X] Join configured xchannels
- [ ] Handle private messages
- [X] Load news list from a JSON file
- [X] Write news list to a JSON file
- [X] Fetch and parse RSS feeds
- [X] Post RSS news
- [ ] Handle IRC disconnects

# Notes

To start a local IRC server :
```sh
docker run --rm --name inspircd -p 6667:6667 -e "INSP_ENABLE_DNSBL=no" -e "INSP_SERVER_NAME=irc.example.com" inspircd/inspircd-docker --debug
```

# IRC Numerics
https://modern.ircdocs.horse/#numerics