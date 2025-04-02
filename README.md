# music-server

> [!NOTE]
> This was originally intended to serve as a replacement to TRIA's manual audio verification. I attempted to create a system that would functionally be very similar to FE2.io, being a server that interfaces with WebSockets and relays audio URLs to clients based on username. However after I had finished implementing the entire backend (including fe2io-rs) and moved onto trying to interface with TRIA maps, I found out that Runtime blocks HTTPService making TRIA maps basically sandboxed from the outside world entirely. This mainly exists as for archival purposes now, or for random updates to the code structure.
> 
> Also in the unlikely case that TRIA's Runtime allows PostAsync in the future (which is really all that is needed for this program to work), this would be able to serve as an FE2.io-like server for any Flood Escape game

Simple music server that listens for JSON POST via HTTP, then relays playback information to the music client

## Usage

Run `music-server`. By default, this program will listen for clients through port 8081 on WebSockets, and POST requests through port 8082 on HTTP. For extra configuration options, run `music-server --help`. To send a music URL or status update, POST with JSON with the format:
```json
{
    "username":"foo",
    "msg_type":"bar",
    "audio_url":"baz",
    "status_type":"qux"
}
```
- `username` is the player's username, and it is a required field. This is case-sensitive.
- `msg_type` is the type of the message, and can be one of "bgm" or "gameStatus". It is also required. 
- `audio_url` is the URL that points to the audio. It is required only if msg_type == "bgm".
- `status_type` is the changed status of the player. It is required only if msg_type == "gameStatus". 

Optionally, you can also send the message in lowerCamelCase. This is intended to be used with [fe2io-rs](https://github.com/some100/fe2io-rs), which also works with this program.

## Use case

If for whatever reason you wanted to reimplement something similar to FE2.io in your game. This was made for usage with ROBLOX's PostAsync function, but it'll work with anything that can HTTP POST. This can be used for if you wanted to create a community map testing game, similar to FE2CM or TRIA, and don't want to go through the effort of verifying every audio that possibly exists.

## Compilation

music-server is written in Rust, so the Cargo toolchain is required for compilation
```
git clone https://github.com/some100/music-server.git
cargo build -r
```
