use termios::*;
use std::fs::File;
use std::io::{stdout, Write};
use std::os::unix::io::AsRawFd;


pub fn terminal_control_raw_mode() -> Result<Termios, std::io::Error> {
    let tty_file: File = File::open("/dev/tty")?;
    let tty_data_original: Termios = Termios::from_fd(tty_file.as_raw_fd())?;
    
    
    let mut tty_raw: Termios = tty_data_original;
    tty_raw.c_lflag &= !(ECHO | ICANON | IEXTEN);
    tty_raw.c_iflag &= !(IXON | ICRNL  | BRKINT | INPCK | ISTRIP);
    tty_raw.c_oflag &= !(OPOST);
    tty_raw.c_cflag |= CS8;
    
    tty_raw.c_cc[VTIME] = 1;
    tty_raw.c_cc[VMIN] = 0;

    tcsetattr(tty_file.as_raw_fd(), TCSAFLUSH, &tty_raw)?;

    print!("\x1B[?25l");
    print!("\x1B[s");
    print!("\x1B[?47h");
    print!("\x1B[?1049h");
    stdout().flush()?;
    
    Ok(tty_data_original)
}

pub fn terminal_control_default_mode(tty: &mut Termios) -> Result<(), std::io::Error> {
    print!("\x1B[?1049l");
    print!("\x1B[?47l");
    print!("\x1B[u");
    print!("\x1B[?25h");
    stdout().flush()?;

    let tty_file: File = File::open("/dev/tty")?;
    tcsetattr(tty_file.as_raw_fd(), TCSAFLUSH, tty)?;

    Ok(())
}

#[inline(always)]
pub fn terminal_tui_clear() -> Result<(), std::io::Error> {
    /* Safely clear screen by moving cursor to 0,0 and then clearing the rest.
     * If \x1B[2J is used then a stack smashing error occurs when displaying an
     * image. Bug??? */
    print!("\x1B[s\x1B[1;1H\x1B[0J\x1B[u");
    stdout().flush()?;

    Ok(())
}