use std::{io::{stdin, Read, Stdin}, sync::atomic::Ordering, thread};

use crossbeam_channel::{unbounded, Receiver};

use crate::{drivers::input::{GraphicsResponse, StdinDFA, StdinInput, TerminalKey}, globals::RUNNING};

pub fn spawn() -> (Receiver<TerminalKey>, Receiver<GraphicsResponse>) {
    let (sender_key, receive_key) = unbounded::<TerminalKey>();
    let (sender_gr, receive_gr) = unbounded::<GraphicsResponse>();

    thread::spawn(move || {
        let mut dfa: StdinDFA = StdinDFA::new();
        let mut handle: Stdin = stdin();
        let mut b: [u8; 1] = [0u8];
        while RUNNING.load(Ordering::Acquire) {
            handle.read(&mut b).unwrap();
            let token: Option<StdinInput> = dfa.feed(b[0]);

            if token.is_none() {
                continue;
            }
            match token.unwrap() {
                StdinInput::TerminalKey(x) => {
                    /* Ignore send errors since they occur only when trying to
                     * close the application */
                    let _ = sender_key.send(x);
                }
                StdinInput::GraphicsResponse(x) => {
                    /* Ignore send errors since they occur only when trying to
                     * close the application */
                    let _ = sender_gr.send(x);
                }
                _ => (),
            }
        }
    });

    return (receive_key, receive_gr);
}