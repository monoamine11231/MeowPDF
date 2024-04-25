use nix::sys::termios::*;
use nix::libc;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::mem::MaybeUninit;

#[derive(Clone, Copy)]
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

pub fn terminal_tui_key_iter() -> impl Iterator
        <Item = Result<TerminalKey, std::io::Error>
    > {
    stdin().bytes().map(|x| {
        let res: Result<TerminalKey, std::io::Error> = match x {
            Ok(0x03) => Ok(TerminalKey::CTRLC),
            Ok(0x04) => Ok(TerminalKey::CTRLD),
            Ok(c) => Ok(TerminalKey::KEY(c)),
            Err(x) => Err(x)
        };
        return res;
    })
}

mod ioctl {
    use nix::ioctl_read_bad;
    use nix::libc;

    ioctl_read_bad!(terminal_size, libc::TIOCGWINSZ, libc::winsize);
}

pub fn terminal_tui_get_dimensions() -> Result<libc::winsize, &'static str> {
    let mut sz: libc::winsize;
    let res: nix::Result<libc::c_int>;
    unsafe {
        sz = MaybeUninit::zeroed().assume_init();
        res = ioctl::terminal_size(1, &mut sz);
    }

    let ret = match res {
        Ok(_) => Ok(sz),
        Err(_) => Err("Error when trying to fetch terminal size")
    };
    ret
}