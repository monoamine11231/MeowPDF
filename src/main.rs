mod tui;
use crate::tui::*;

mod graphics;
use crate::graphics::*;

use std::ops::Deref;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{io::Write, io::stdout, io::Stdout, sync::Mutex, thread};
use nix::libc;
use nix::sys::termios::Termios;


fn main() {
    let tty_data_original_main: Termios = terminal_control_raw_mode()
        .expect("Error when setting terminal to raw mode");
    let tty_data_original_panic_hook: Mutex<Termios> = Mutex::from(
        tty_data_original_main.clone());


    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let tty = tty_data_original_panic_hook.lock().unwrap();
        /* Atleast try to cook the terminal on error before printing the message. 
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty.deref());
        default_panic(info);
    }));


    let (sender_winsize, receive_winsize) = channel::<libc::winsize>();
    thread::spawn(move || {
        let mut wz: libc::winsize = terminal_tui_get_dimensions().unwrap();
        loop {
            thread::sleep(Duration::from_millis(100));

            let tmp: libc::winsize = terminal_tui_get_dimensions().unwrap();
            if tmp == wz {
                continue;
            }

            wz = tmp;
            /* Notify that the terminal window has been changed */
            sender_winsize
                .send(wz)
                .expect("Could not send a terminal window size change signal");
        }
    });

    terminal_tui_clear()
        .expect("Error when clearing the screen");

    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    let mut handle: Stdout = stdout();
    handle.flush()
        .expect("Error when flushing stdout");

    let mut i = 0;

    let mut key_iter = terminal_tui_key_iter().peekable();
    let mut winsize_iter = receive_winsize.try_iter().peekable();

    loop {
        /* Break the loop if a CTRL-C or similar sigint signal was sent */
        if key_iter.peek().is_some() {
            let key = *key_iter.peek().unwrap().as_ref().unwrap();
            if handle_key(key) {
                break;
            }
            key_iter.next();
        }

        /* Rerender on terminal window size change */
        if winsize_iter.peek().is_some() {
            winsize_iter.next();
        }


        terminal_tui_clear()
            .expect("Error when clearing the screen");
    

        print!("\x1B[1;1H");
        print!("i {}",i);
        i += 1;

        stdout()
            .flush()
            .expect("Could not flush stdout.");

        while key_iter.peek().is_none() && winsize_iter.peek().is_none() {
            winsize_iter.next();
            key_iter.next();
        }
    }

    terminal_control_default_mode(&tty_data_original_main)
        .expect("Error when setting terminal to default mode");   
}

/* `true` indicates that the caller should exit *safely* the current process */
fn handle_key(key: TerminalKey) -> bool {
    let res: bool = match key {
        TerminalKey::CTRLC | TerminalKey::CTRLD |
        TerminalKey::KEY(b'q') | TerminalKey::KEY(b'Q') => true,
        _ => false
    };
    return res;
}
