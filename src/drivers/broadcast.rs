use crossbeam_channel::{unbounded, Receiver, SendError, Sender};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};


/* Big thanks to this guy for this solution,
 * https://github.com/crossbeam-rs/crossbeam/issues/374#issuecomment-643378762 */
#[derive(Clone)]
pub struct UnboundedBroadcast<T> {
    channels: Arc<RwLock<Vec<Sender<T>>>>,
}

impl<T: 'static + Clone + Send + Sync> UnboundedBroadcast<T> {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn subscribe(&mut self) -> Receiver<T> {
        let (sender, receiver): (Sender<T>, Receiver<T>) = unbounded::<T>();
        let mut channels_lock: RwLockWriteGuard<Vec<Sender<T>>> =
            self.channels.write().unwrap();

        channels_lock.push(sender);
        return receiver;
    }

    pub fn send(&self, message: T) -> Result<(), SendError<T>> {
        let channels_lock: RwLockReadGuard<Vec<Sender<T>>> =
            self.channels.read().unwrap();

        for channel in channels_lock.iter() {
            channel.send(message.clone())?;
        }

        Ok(())
    }
}
