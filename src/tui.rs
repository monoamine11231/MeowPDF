use termios::*;
use std::fs::File;
use std::io::{stdout, stdin, Read, Write};
use std::os::unix::io::AsRawFd;


pub enum TerminalKey {
    CTRLC,
    CTRLD,
    KEY(u8)    
}

pub fn terminal_control_raw_mode() -> Result<Termios, &'static str> {
    let tty_file: File = File::open("/dev/tty")
        .expect("Could not open /dev/tty.");
    let tty_data_original: Termios = Termios::from_fd(tty_file.as_raw_fd())
        .expect("Could not load `termios` struct from /dev/tty");
    
    
    let mut tty_raw: Termios = tty_data_original;
    tty_raw.c_lflag &= !(ECHO | ICANON | ISIG   | IEXTEN);
    tty_raw.c_iflag &= !(IXON | ICRNL  | BRKINT | INPCK | ISTRIP);
    tty_raw.c_oflag &= !(OPOST);
    tty_raw.c_cflag |= CS8;
    
    tty_raw.c_cc[VTIME] = 1;
    tty_raw.c_cc[VMIN] = 0;

    tcsetattr(tty_file.as_raw_fd(), TCSAFLUSH, &tty_raw)
        .expect("Could not set `termios` struct to /dev/tty");

    print!("\x1B[?25l");
    print!("\x1B[s");
    print!("\x1B[?47h");
    print!("\x1B[?1049h");
    stdout()
        .flush()
        .expect("Could not flush stdout.");
    
    Ok(tty_data_original)
}

pub fn terminal_control_default_mode(tty: &Termios) -> Result<(), &'static str> {
    print!("\x1B[?1049l");
    print!("\x1B[?47l");
    print!("\x1B[u");
    print!("\x1B[?25h");
    stdout()
        .flush()
        .expect("Could not flush stdout.");

    let tty_file: File = File::open("/dev/tty")
        .expect("Could not open /dev/tty.");
    tcsetattr(tty_file.as_raw_fd(), TCSAFLUSH, tty)
        .expect("Could not set `termios` struct to /dev/tty.");

    Ok(())
}

#[inline(always)]
pub fn terminal_tui_clear() -> Result<(), &'static str> {
    /* Safely clear screen by moving cursor to 0,0 and then clearing the rest.
     * If \x1B[2J is used then a stack smashing error occurs when displaying an
     * image. Bug??? */
    print!("\x1B[s\x1B[1;1H\x1B[0J\x1B[u");
    stdout()
        .flush()
        .expect("Could not flush stdout.");

    Ok(())
}

pub fn terminal_tui_get_key() -> Result<Option<TerminalKey>, &'static str> {
    let mut buf: [u8;1] = [b'\0'];

    let read: Result<usize, std::io::Error> = stdin().read(&mut buf);
    if read.is_err() {
        return Err("Could not read from stdin.");
    }

    if read.is_ok_and(|x| x == 0) {
        return Ok(None);
    }

    let key: Option<TerminalKey> = match buf[0] {
        0x03 => Some(TerminalKey::CTRLC),
        0x04 => Some(TerminalKey::CTRLD),
        c => Some(TerminalKey::KEY(c))
    };
    Ok(key)
}

/* Read a key to `key` while at the same time returning a signal whether a key was
 * pressed or not */
pub fn terminal_tui_has_key(key: &mut Option<TerminalKey>) -> Result<bool, &'static str>{
    let res: Result<bool, &'static str> = terminal_tui_get_key()
        .map(move |x| {
            *key = x;
            return (*key).is_some();
        });
    res
}