mod tui;
use crate::tui::*;

mod graphics;
use crate::graphics::*;

use std::io::Read;
use std::{io::Write, io::stdout, io::stdin, io::Stdout, sync::Arc, thread, time::Duration};
use termios::Termios;


fn main() {
    let tty_data_original: Arc<Termios> = Arc::new(
        terminal_control_raw_mode()
        .expect("Error when setting terminal to raw mode")
    );

    let tty_data_original_panic_hook: Arc<Termios> = tty_data_original.clone();

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        /* Atleast try to cook the terminal on error before printing the message. 
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty_data_original_panic_hook.as_ref());
        default_panic(info);
    }));

    terminal_tui_clear()
        .expect("Error when clearing the screen");

    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    let mut handle: Stdout = stdout();
    handle.flush()
        .expect("Error when flushing stdout");

    let mut key: Option<TerminalKey> = None;
    let mut pressed_keys: i32 = 0;
    loop {
        /* Break the loop if a CTRL-C or similar sigint signal was sent */
        if handle_key(&key) {
            break;
        }

        terminal_tui_clear()
            .expect("Error when clearing the screen");

            print!("\x1B[1;1H");
        print!("# of pressed keys: {}", pressed_keys);
        pressed_keys += 1;

        stdout()
            .flush()
            .expect("Could not flush stdout.");

        while !terminal_tui_has_key(&mut key).unwrap() {}
    }

    terminal_control_default_mode(&tty_data_original)
        .expect("Error when setting terminal to default mode");   
}

/* `true` indicates that the caller should exit *safely* the current process */
fn handle_key(key: &Option<TerminalKey>) -> bool {
    let res: bool = match key {
        Some(TerminalKey::CTRLC) | Some(TerminalKey::CTRLD) => true,
        Some(_) | None => false
    };
    return res;
}