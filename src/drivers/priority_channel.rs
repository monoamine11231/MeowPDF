use crossbeam_channel::{unbounded, Receiver, RecvError, Select, SendError, Sender, TryRecvError, TrySendError};

use crate::clear_channel;

#[derive(Clone)]
pub struct PrioritySender<T, const U: usize> {
    senders: [Sender<T>; U],
}

impl<T, const U: usize> PrioritySender<T, U> {
    #[allow(dead_code)]
    pub fn send_priority(&self, obj: T, priority: usize) -> Result<(), SendError<T>> {
        Ok(self.senders[priority].send(obj)?)
    }

    #[allow(dead_code)]
    pub fn try_send_priority(&self, obj: T, priority: usize) -> Result<(), TrySendError<T>> {
        Ok(self.senders[priority].try_send(obj)?)
    }
}

#[derive(Clone)]
pub struct PriorityReceiver<T, const U: usize> {
    receivers: [Receiver<T>; U],
}

impl<T, const U: usize> PriorityReceiver<T, U> {
    #[allow(dead_code)]
    pub fn construct_select<'a>(&'a self) -> Select<'a> {
        let mut sel: Select<'a> = Select::new();
        for i in 0..U {
            sel.recv(&self.receivers[i]);
        }

        sel
    }

    #[allow(dead_code)]
    pub fn construct_biased_select<'a>(&'a self) -> Select<'a> {
        let mut sel: Select<'a> = Select::new_biased();
        for i in 0..U {
            sel.recv(&self.receivers[i]);
        }

        sel
    }

    #[allow(dead_code)]
    pub fn recv_priority(&self, priority: usize) -> Result<T, RecvError> {
        Ok(self.receivers[priority].recv()?)
    }

    #[allow(dead_code)]
    pub fn try_recv_priority(&self, priority: usize) -> Result<T, TryRecvError> {
        Ok(self.receivers[priority].try_recv()?)
    }

    #[allow(dead_code)]
    pub fn clear_priority(&self, priority: usize) {
        clear_channel!(self.receivers[priority]);
    }
}

pub fn unbounded_priority<T: Clone, const U: usize>(
) -> (PrioritySender<T, U>, PriorityReceiver<T, U>) {
    let mut pr_senders = Vec::<Sender<T>>::new();
    let mut pr_receivers = Vec::<Receiver<T>>::new();

    for _ in 0..U {
        let (sender, receiver) = unbounded::<T>();
        pr_senders.push(sender);
        pr_receivers.push(receiver);
    }

    let pr_sender = PrioritySender::<T, U> {
        senders: pr_senders.try_into().ok().unwrap(),
    };

    let pr_receiver = PriorityReceiver::<T, U> {
        receivers: pr_receivers.try_into().ok().unwrap(),
    };

    (pr_sender, pr_receiver)
}
