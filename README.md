This project is a rewrite of [gruik](https://gitlab.com/gcu-squad/gruik/) in rust

I started it to learn rust.

To be complete, the following features are needed :

- [X] Get an optionnal argument
- [X] Read a yaml config file
- [ ] Hot reloading of the configuration when the yaml file is changed
- [ ] Connect to an IRC server
- [ ] Join configured xchannels
- [ ] Handle private messages
- [ ] Fetch and post news from RSS feeds
- [ ] Handle IRC disconnects

# Notes

To start a local IRC server :
```sh
docker run --name inspircd -p 6667:6667 -e "INSP_ENABLE_DNSBL=no" -e "INSP_SERVER_NAME=irc.example.com" inspircd/inspircd-docker
```