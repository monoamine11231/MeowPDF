use nix::sys::termios::*;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};


pub enum TerminalKey {
    CTRLC,
    CTRLD,
    KEY(u8)    
}

pub fn terminal_control_raw_mode() -> Result<Termios, &'static str> {
    let tty_file_1: File = File::open("/dev/tty")
        .expect("Could not open /dev/tty.");
    let tty_data_original: Termios = tcgetattr(tty_file_1)
        .expect("Could not load `termios` struct from /dev/tty");
    
    
    let mut tty_raw: Termios = tty_data_original.clone();
    tty_raw.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON |
        LocalFlags::ISIG   | LocalFlags::IEXTEN);
    tty_raw.input_flags &= !(InputFlags::IXON | InputFlags::ICRNL  |
        InputFlags::BRKINT | InputFlags::INPCK | InputFlags::ISTRIP);
    tty_raw.output_flags &= !(OutputFlags::OPOST);
    tty_raw.control_flags |= ControlFlags::CS8;
    
    tty_raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 1;
    tty_raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 0;

    let tty_file_2: File = File::open("/dev/tty")
        .expect("Could not open /dev/tty.");
    tcsetattr(tty_file_2, SetArg::TCSAFLUSH, &tty_raw)
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
    tcsetattr(tty_file, SetArg::TCSAFLUSH, tty)
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

pub fn terminal_tui_get_dimensions() -> Result<(u32, u32), &'static str> {
    
    
    Ok((0,0))
}