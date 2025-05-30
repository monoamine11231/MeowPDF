use core::fmt;
use crossterm::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearImages;
impl Command for ClearImages {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1B_Ga=d,d=a\x1B\\")
    }
}

/* A small hack to get cursor position in pixels
 * Replacing ?1006 with ?1016h reports cursor position in pixels instead of cells */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableMouseCapturePixels;
impl Command for EnableMouseCapturePixels {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(concat!(
            "\x1B[?1000h",
            "\x1B[?1002h",
            "\x1B[?1003h",
            "\x1B[?1015h",
            "\x1B[?1016h",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableMouseCapturePixels;
impl Command for DisableMouseCapturePixels {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str(concat!(
            "\x1B[?1016l",
            "\x1B[?1015l",
            "\x1B[?1003l",
            "\x1B[?1002l",
            "\x1B[?1000l",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
#[allow(dead_code)]
pub enum PointerShape {
    Default,
    Pointer,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetPointerShape(pub PointerShape);
impl Command for SetPointerShape {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        const LOOKUP: [&str; 3] = ["", "pointer", "text"];
        write!(f, "\x1B]22;{}\x1B\\", LOOKUP[self.0 as usize])
    }
}
