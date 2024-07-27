use nix::pty::Winsize;
use nix::sys::termios::*;
use std::fs::File;
use std::io::{stdout, Stdout, Write};
use std::mem::MaybeUninit;

pub fn terminal_control_raw_mode() -> Result<Termios, String> {
    let tty_file_1: File =
        File::open("/dev/tty").map_err(|x| format!("Could not open /dev/tty: {}", x))?;
    let tty_data_original: Termios = tcgetattr(tty_file_1)
        .map_err(|x| format!("Could not load `termios` struct from /dev/tty: {}", x))?;

    let mut tty_raw: Termios = tty_data_original.clone();
    tty_raw.local_flags &=
        !(LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::ISIG | LocalFlags::IEXTEN);
    tty_raw.input_flags &= !(InputFlags::IXON
        | InputFlags::ICRNL
        | InputFlags::BRKINT
        | InputFlags::INPCK
        | InputFlags::ISTRIP);
    tty_raw.output_flags &= !(OutputFlags::OPOST);
    tty_raw.control_flags |= ControlFlags::CS8;

    tty_raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0;
    tty_raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;

    let tty_file_2: File =
        File::open("/dev/tty").map_err(|x| format!("Could not open /dev/tty: {}", x))?;
    tcsetattr(tty_file_2, SetArg::TCSAFLUSH, &tty_raw)
        .map_err(|x| format!("Could not set `termios` struct to /dev/tty: {}", x))?;

    let mut handle: Stdout = stdout();
    handle
        .write(b"\x1B[?25l\x1B[s\x1B[?47h\x1B[?1049;1003;1006h")
        .unwrap();
    handle.flush().unwrap();

    Ok(tty_data_original)
}

pub fn terminal_control_default_mode(tty: &Termios) -> Result<(), String> {
    let mut handle: Stdout = stdout();
    handle
        .write(b"\x1B[?1003;1006;1049l\x1B[?47l\x1B[u\x1B[?25h")
        .unwrap();
    handle.flush().unwrap();

    let tty_file: File =
        File::open("/dev/tty").map_err(|x| format!("Could not open /dev/tty: {}", x))?;
    tcsetattr(tty_file, SetArg::TCSAFLUSH, tty)
        .map_err(|x| format!("Could not set `termios` struct to /dev/tty: {}", x))?;

    Ok(())
}

#[inline(always)]
/* Clears screen without freeing image memory */
pub fn terminal_tui_clear() {
    /* Safely clear screen by moving cursor to 0,0 and then clearing the rest.
     * If \x1B[2J is used then a stack smashing error occurs when displaying an
     * image. Bug??? */
    let mut handle: Stdout = stdout();
    handle.write(b"\x1B[s\x1B[1;1H\x1B[0J\x1B[u").unwrap();
    handle.write(b"\x1B_Ga=d,d=a\x1B\\").unwrap();
    handle.flush().unwrap();
}

pub fn terminal_tui_get_dimensions() -> Result<Winsize, String> {
    let mut sz: Winsize;
    let res: nix::Result<i32>;
    unsafe {
        sz = MaybeUninit::zeroed().assume_init();
        res = ioctl::terminal_size(1, &mut sz);
    }

    let ret = match res {
        Ok(_) => Ok(sz),
        Err(x) => Err(format!(
            "Error when trying to fetch terminal size: ERRNO {}",
            x
        )),
    };
    ret
}

mod ioctl {
    use nix::{ioctl_read_bad, pty::Winsize};
    /* Big thanks to
     * https://github.com/nix-rust/nix/issues/201#issuecomment-154902042 */
    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    const TIOCGWINSZ: libc::c_ulong = 0x40087468;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TIOCGWINSZ: u64 = 0x5413;

    ioctl_read_bad!(terminal_size, TIOCGWINSZ, Winsize);
}
