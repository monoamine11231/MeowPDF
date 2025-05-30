use std::{sync::atomic::Ordering, thread};

use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{
    read, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, MouseEvent,
    MouseEventKind,
};

use crate::{drivers::graphics::GraphicsResponse, globals::RUNNING};

pub struct EventThreadData(
    pub Receiver<KeyEvent>,
    pub Receiver<MouseEvent>,
    pub Receiver<GraphicsResponse>,
    pub Receiver<(u16, u16)>,
);

pub fn spawn() -> EventThreadData {
    let (sender_key, receive_key) = unbounded::<KeyEvent>();
    let (sender_mouse, receive_mouse) = unbounded::<MouseEvent>();
    let (sender_gr, receive_gr) = unbounded::<GraphicsResponse>();
    let (sender_ws, receive_ws) = unbounded::<(u16, u16)>();

    thread::spawn(move || {
        while RUNNING.load(Ordering::Acquire) {
            match read().expect("Could not read event") {
                Event::Key(event) => {
                    sender_key
                        .try_send(event)
                        .expect("Could not send key event");
                }
                Event::ApplicationProgramCommand(command) => {
                    sender_gr
                        .try_send(GraphicsResponse::new(command.as_bytes()))
                        .expect("Could not send graphics response");
                }
                Event::Mouse(event) => match event {
                    MouseEvent {
                        kind: MouseEventKind::ScrollUp,
                        modifiers,
                        ..
                    } => {
                        sender_key
                            .try_send(KeyEvent {
                                code: KeyCode::Down,
                                modifiers,
                                kind: KeyEventKind::Press,
                                state: KeyEventState::NONE,
                            })
                            .expect("Could not send key event");
                        sender_mouse.try_send(event).expect("Could not send mouse");
                    }
                    MouseEvent {
                        kind: MouseEventKind::ScrollLeft,
                        modifiers,
                        ..
                    } => {
                        sender_key
                            .try_send(KeyEvent {
                                code: KeyCode::Right,
                                modifiers,
                                kind: KeyEventKind::Press,
                                state: KeyEventState::NONE,
                            })
                            .expect("Could not send key event");
                        sender_mouse.try_send(event).expect("Could not send mouse");
                    }
                    MouseEvent {
                        kind: MouseEventKind::ScrollRight,
                        modifiers,
                        ..
                    } => {
                        sender_key
                            .try_send(KeyEvent {
                                code: KeyCode::Left,
                                modifiers,
                                kind: KeyEventKind::Press,
                                state: KeyEventState::NONE,
                            })
                            .expect("Could not send key event");
                        sender_mouse.try_send(event).expect("Could not send mouse");
                    }
                    MouseEvent {
                        kind: MouseEventKind::ScrollDown,
                        modifiers,
                        ..
                    } => {
                        sender_key
                            .try_send(KeyEvent {
                                code: KeyCode::Up,
                                modifiers,
                                kind: KeyEventKind::Press,
                                state: KeyEventState::NONE,
                            })
                            .expect("Could not send key event");
                        sender_mouse.try_send(event).expect("Could not send mouse");
                    }
                    x => {
                        sender_mouse
                            .try_send(x)
                            .expect("Could not send mouse event");
                    }
                },
                Event::Resize(width, height) => {
                    sender_ws
                        .try_send((width, height))
                        .expect("Could not send new window dimensions");
                }
                _ => (),
            }
        }
    });

    EventThreadData(receive_key, receive_mouse, receive_gr, receive_ws)
}
