use std::{sync::atomic::Ordering, thread};

use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{read, Event, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::{drivers::graphics::GraphicsResponse, globals::RUNNING};

pub enum InputEvent {
    Key(KeyEvent),
    MouseScroll(MouseEventKind, KeyModifiers),
}

pub struct EventThreadData(
    pub Receiver<InputEvent>,
    pub Receiver<MouseEvent>,
    pub Receiver<GraphicsResponse>,
    pub Receiver<(u16, u16)>,
);

pub fn spawn() -> EventThreadData {
    let (sender_input, receive_input) = unbounded::<InputEvent>();
    let (sender_mouse, receive_mouse) = unbounded::<MouseEvent>();
    let (sender_gr, receive_gr) = unbounded::<GraphicsResponse>();
    let (sender_ws, receive_ws) = unbounded::<(u16, u16)>();

    thread::spawn(move || {
        while RUNNING.load(Ordering::Acquire) {
            match read().expect("Could not read event") {
                Event::Key(event) => {
                    sender_input
                        .try_send(InputEvent::Key(event))
                        .expect("Could not send key event");
                }
                Event::ApplicationProgramCommand(command) => {
                    sender_gr
                        .try_send(GraphicsResponse::new(command.as_bytes()))
                        .expect("Could not send graphics response");
                }
                Event::Mouse(event) => match event {
                    MouseEvent {
                        kind:
                            kind @ (MouseEventKind::ScrollUp
                            | MouseEventKind::ScrollDown
                            | MouseEventKind::ScrollLeft
                            | MouseEventKind::ScrollRight),
                        modifiers,
                        ..
                    } => {
                        sender_input
                            .try_send(InputEvent::MouseScroll(kind, modifiers))
                            .expect("Could not send mouse scroll event");
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

    EventThreadData(receive_input, receive_mouse, receive_gr, receive_ws)
}
