use std::{
    sync::{atomic::Ordering, RwLockWriteGuard},
    thread,
    time::Duration,
};

use crossbeam_channel::{unbounded, Receiver};
use nix::pty::Winsize;

use crate::{
    drivers::tui::terminal_tui_get_dimensions,
    globals::{RUNNING, TERMINAL_SIZE},
};

pub fn spawn() -> Receiver<()> {
    let (sender_winsize, receiver_winsize) = unbounded::<()>();

    thread::spawn(move || {
        let mut wz: Winsize = terminal_tui_get_dimensions().unwrap();
        while RUNNING.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(100));

            let tmp: Winsize = terminal_tui_get_dimensions().unwrap();
            if tmp == wz {
                continue;
            }

            wz = tmp;
            /* Notify that the terminal window has been changed */
            sender_winsize
                .send(())
                .expect("Could not send a terminal window size change signal");

            /* Change terminal window size global variable */
            let mut terminal_size_lock: RwLockWriteGuard<Winsize> =
                TERMINAL_SIZE.get().unwrap().write().unwrap();
            *terminal_size_lock = wz;
        }
    });

    return receiver_winsize;
}
