mod tui;
use crate::tui::*;

mod graphics;
use crate::graphics::*;

use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{io::Write, io::stdout, sync::Mutex, thread};

use nix::libc;
use nix::sys::termios::Termios;
use mupdf::{document::Document, Page};

use notify::RecursiveMode;
use notify::{Watcher, Result, event::{ModifyKind, DataChange}};


fn main() {
    /* ============================= Uncook the terminal ============================= */
    let tty_data_original_main: Termios = terminal_control_raw_mode()
        .expect("Error when setting terminal to raw mode");
    let tty_data_original_panic_hook: Mutex<Termios> = Mutex::from(
        tty_data_original_main.clone());

    /* ========================== Cook the terminal on panic ========================= */
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let tty = tty_data_original_panic_hook.lock().unwrap();
        /* Atleast try to cook the terminal on error before printing the message. 
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty.deref());
        default_panic(info);
    }));

    /* ========== Check if the terminal supports the Kitty graphics protocol ========= */
    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    /* ==================== Thread notifying terminal size change ==================== */
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

    /* ========================= Thread notifying file change ======================== */
    let (sender_file, receive_file) = channel::<bool>();
    let mut watcher_file = notify::recommended_watcher(move|res: Result<notify::Event>| {
        let event: notify::Event = res
            .expect("Could not watch file changes for the given file");
        
        match event.kind {
            notify::EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                sender_file
                    .send(true)
                    .expect("Could not send a file change signal");
            },
            _ => ()
        }
    })
        .expect("Could not initialize a file watcher for the given file");

    watcher_file
        .watch(Path::new("test.txt"), RecursiveMode::NonRecursive)
        .expect("Could not start watching file changes for the given file");


    /* ============================== Main program loop ============================== */
    let file: String = std::env::args().nth(1)
        .expect("No provided pdf!");

    let mut app: AppState = AppState::init(file);


    terminal_tui_clear()
        .expect("Error when clearing the screen");

    let mut i = 0;
    let mut key_iter = terminal_tui_key_iter().peekable();
    let mut winsize_iter = receive_winsize.try_iter().peekable();
    let mut file_iter = receive_file.try_iter().peekable();
    
    loop {
        /* Break the loop if a CTRL-C or similar sigint signal was sent */
        if chan_has!(key_iter) {
            let key: TerminalKey = **key_iter.peek().as_ref().unwrap();
            if handle_key(key) {
                break;
            }
            key_iter.next();
        }

        /* Rerender on terminal window size change */
        if chan_has!(winsize_iter) {
            winsize_iter.next();
        }

        /* Do something on file change */
        if chan_has!(file_iter) {
            file_iter.next();
        }


        terminal_tui_clear()
            .expect("Error when clearing the screen");
    

        print!("\x1B[1;1H");
        print!("i {}",i);
        i += 1;

        stdout()
            .flush()
            .expect("Could not flush stdout.");

        while !chan_has!(key_iter) && !chan_has!(winsize_iter) && !chan_has!(file_iter) {
            key_iter.next();
            winsize_iter.next();
            file_iter.next();
        }
    }

    /* ========================== Cook the terminal on exit ========================== */
    terminal_control_default_mode(&tty_data_original_main)
        .expect("Error when setting terminal to default mode");   
}


/* `true` indicates that the caller should exit *safely* the current process */
fn handle_key(key: TerminalKey) -> bool {
    let res: bool = match key {
        TerminalKey::CTRLC | TerminalKey::CTRLD |
        TerminalKey::OTHER(b'q') | TerminalKey::OTHER(b'Q') => true,
        _ => false
    };
    return res;
}


struct AppState {
    file: String,
    document: Document,
    cache: HashMap<u32, Page>,
    /* Offset is given in page width and page height units */
    offset: (f32, f32)
}

impl AppState {
    fn init(file: String) -> Self {
        let document: Document = Document::open(file.as_str())
            .expect("Could not open the given PDF file for reading");

        if !document.is_pdf() {
            panic!("The given PDF file is not a PDF!");
        }

        Self {
            file: file,
            document: document,
            cache: HashMap::new(),
            offset: (0.0f32, 0.0f32)
        }
    }
}


#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}
