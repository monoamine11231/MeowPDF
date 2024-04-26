use nix::sys::termios::*;
use nix::libc;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::mem::MaybeUninit;


#[derive(Debug, Clone, Copy)]
pub enum TerminalKey {
    UP,
    LEFT,
    RIGHT,
    DOWN,
    CTRLC,
    CTRLD,
    OTHER(u8)    
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

pub fn terminal_tui_key_iter() -> impl Iterator<Item = TerminalKey> {
    let mut escape: bool = false;
    let mut window: Vec<u8> = Vec::new();
    stdin().bytes().filter_map(move |x| {
        let b: u8 = x.expect("Could not read a byte from stdin"); 

        if !escape {
            escape = b == b'\x1B';
        }

        window.push(b);
        let res: Option<TerminalKey> = match window.as_slice() {
            [b'\x03'] => Some(TerminalKey::CTRLC),
            [b'\x04'] => Some(TerminalKey::CTRLD),

            [b'\x1B', b'[', b'A'] => Some(TerminalKey::UP),
            [b'\x1B', b'[', b'B'] => Some(TerminalKey::DOWN),
            [b'\x1B', b'[', b'C'] => Some(TerminalKey::RIGHT),
            [b'\x1B', b'[', b'D'] => Some(TerminalKey::LEFT),

            c if !escape => Some(TerminalKey::OTHER(c[0])),           
            _ => None
        };

        if !escape {
            window.clear();
        }

        if escape && res.is_some() {
            escape = false;
            window.clear();
        }

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