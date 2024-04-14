mod tui;
use crate::tui::*;

mod graphics;
use crate::graphics::*;

use std::{io::Write, io::stdout, io::Stdout, thread, time::Duration};
use termios::Termios;


fn main() -> Result<(), &'static str>{
    let mut tty_data_original: Termios = terminal_control_raw_mode()
        .expect("Could not set terminal to raw mode.");

    terminal_tui_clear()
        .expect("Could not clear the screen");

    terminal_graphics_test_support()?;

    let mut handle: Stdout = stdout();
    handle.flush()
        .expect("Could not flush stdout.");

    terminal_graphics_apc_success()
        .expect("Could not transfer image to terminal.");

    for _ in 0..4 {
        terminal_tui_clear()
            .expect("Could not clear the screen");

        print!("\x1B[1;1H");
        print!("\x1B_Ga=d\x1B\\");
        print!("\x1B_Gf=100,a=p,z=-1,i=228\x1B\\");
        handle.flush()
            .expect("Could not flush stdout.");
        terminal_graphics_apc_success()
            .expect("Could not display image");

        thread::sleep(Duration::from_secs(1));
    }

    terminal_control_default_mode(&mut tty_data_original)
        .expect("Could not return terminal to default mode.");   

    Ok(())
}