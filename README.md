<<<<<<< HEAD
# music-server

Simple music server that listens for JSON POST via HTTP, then relays playback information to the music client

## Usage

Run `music-server`. By default, this program will listen for all requests through port 8081 on WebSockets, and through port 8082 on HTTP. For extra configuration options, run `music-server --help`. Use with [fe2io-rs](https://github.com/some100/fe2io-rs). To send a music URL or status update, POST with JSON {"username":"a","msg_type":"b","audio_url":"c","status_type":"d"}. Optionally, you can also send the message in lowerCamelCase, for compatibility with FE2.io.

## Use case

If for whatever reason you wanted to reimplement something similar to FE2.io in your game. This was made for usage with ROBLOX's PostAsync function, but it'll work with anything that can HTTP POST.

## Compilation

music-server is written in Rust, so the Cargo toolchain is required for compilation
```
git clone https://github.com/some100/music-server.git
cargo build -r
```
=======
# music-server
>>>>>>> 4699551 (Initial commit)
