use std::{path::Path, sync::OnceLock};

use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::{
    event::{DataChange, ModifyKind},
    RecommendedWatcher, RecursiveMode, Watcher,
};

static SENDER_FILE_RELOAD: OnceLock<Sender<()>> = OnceLock::new();
static WATCHER_FILE: OnceLock<RecommendedWatcher> = OnceLock::new();

pub fn spawn(file: &String) -> Result<Receiver<()>, String> {
    let (sender_file_reload, receiver_file_reload) = unbounded::<()>();

    SENDER_FILE_RELOAD.get_or_init(|| sender_file_reload.clone());

    let mut watcher_file =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event: notify::Event =
                res.expect("Could not watch file changes for the given file");

            match event.kind {
                notify::EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                    (*SENDER_FILE_RELOAD.get().unwrap())
                        .send(())
                        .expect("Could not send a file change signal");
                }
                _ => (),
            }
        })
        .map_err(|x| format!("Could not initialize a file watcher: {}", x))?;

    watcher_file
        .watch(Path::new(file.as_str()), RecursiveMode::NonRecursive)
        .expect("Could not start watching file changes for the given file");

    WATCHER_FILE.get_or_init(|| watcher_file);

    Ok(receiver_file_reload)
}
