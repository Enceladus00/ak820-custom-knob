//! Listens for game plugins (see `integrations/valheim/`) on
//! 127.0.0.1:50820 and passes what they report to the effects thread, which
//! plays the effects whose `game:` situations hold.
//!
//! Only localhost is bound, so nothing outside this PC can reach it.

use std::net::UdpSocket;
use std::thread;

use knob_core::game::{GAME_PORT, parse_line};

use crate::effects::{self, EffectsMsg};

pub fn start() {
    let socket = match UdpSocket::bind(("127.0.0.1", GAME_PORT)) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("game: cannot listen on 127.0.0.1:{GAME_PORT}: {e}; game effects are off");
            return;
        }
    };
    thread::Builder::new()
        .name("game".into())
        .spawn(move || listen(socket))
        .expect("spawn game thread");
}

fn listen(socket: UdpSocket) {
    log::info!("game: listening on 127.0.0.1:{GAME_PORT}");
    let mut buf = [0u8; 1024];
    let mut announced = String::new();
    loop {
        let n = match socket.recv(&mut buf) {
            Ok(n) => n,
            // Windows reports an earlier send's ICMP "port unreachable" here;
            // nothing to do but keep listening.
            Err(e) => {
                log::debug!("game: receive: {e}");
                continue;
            }
        };
        let Ok(text) = std::str::from_utf8(&buf[..n]) else {
            continue;
        };
        for line in text.lines() {
            let Some(line) = parse_line(line) else {
                continue;
            };
            let game = match &line {
                knob_core::game::GameLine::State(s) => &s.game,
                knob_core::game::GameLine::Event { game, .. } => game,
            };
            if *game != announced {
                log::info!("game: {game} is reporting");
                announced = game.clone();
            }
            effects::send(EffectsMsg::Game(line));
        }
    }
}
