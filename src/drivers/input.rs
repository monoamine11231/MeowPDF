use std::{
    collections::HashMap,
    str::{self, Split},
};

use regex_automata::{
    dfa::{
        dense::{self, DFA},
        Automaton,
    },
    util::primitives::StateID,
    Input, MatchError, MatchKind, PatternID,
};

type TokenParser = fn(&[u8]) -> StdinInput;
const REGEX_SET: &[(&str, TokenParser)] = &[
    ("^\x03", parsers::parse_ctrlc),
    ("^\x04", parsers::parse_ctrld),
    ("^[+\\-a-zA-Z0-9]", parsers::parse_alphanumerical),
    ("^\x1B\\[[ABCD]", parsers::parse_udrl_key),
    ("^\x1B\\[<\\d+;\\d+;\\d+[m|M]", parsers::parse_mouse),
    ("^\x1B_G.*\x1B\\\\", parsers::parse_graphics_response),
];

/* Heavily degenerated regex DFA */
pub struct StdinDFA {
    dfa: DFA<Vec<u32>>,
    state: Option<StateID>,
    input: Vec<u8>,
}

impl StdinDFA {
    pub fn new() -> Self {
        let sources: Vec<&str> = REGEX_SET.iter().map(|x| x.0).collect();
        Self {
            dfa: dense::Builder::new()
                .configure(dense::Config::new().match_kind(MatchKind::All))
                .build_many(&sources)
                .unwrap(),
            state: None,
            input: Vec::new(),
        }
    }

    pub fn feed(&mut self, x: u8) -> Option<StdinInput> {
        /* Dead or quit state */
        macro_rules! wall_state {
            ($dfa:expr, $state:expr) => {
                ($dfa.is_dead_state($state) || $dfa.is_quit_state($state))
            };
        }

        macro_rules! compute_start_state {
            () => {
                let start_state: Result<StateID, MatchError> =
                    self.dfa.start_state_forward(&Input::new(&[x]));
                if start_state.is_err() {
                    return None;
                }
                self.state = Some(start_state.unwrap());
            };
        }

        if self.state.is_none() {
            compute_start_state!();
        }
        self.input.push(x);
        self.state = Some(self.dfa.next_state(self.state.unwrap(), x));
        let next_eoi: StateID = self.dfa.next_eoi_state(self.state.unwrap());
        if self.dfa.is_match_state(next_eoi) {
            let index: PatternID = self.dfa.match_pattern(next_eoi, 0);
            let parser: TokenParser = REGEX_SET[index.as_usize()].1;
            let token: StdinInput = parser(self.input.as_slice());

            self.input.clear();
            self.state = None;
            return Some(token);
        }
        if wall_state!(self.dfa, self.state.unwrap()) {
            self.input.clear();
            /* Try to reuse the character which lead to a wall state as a beginning
             * of a new pattern */
            compute_start_state!();
            self.state = Some(self.dfa.next_state(self.state.unwrap(), x));
            /* Matches and wall states are delayed by one state */
            if wall_state!(self.dfa, self.dfa.next_eoi_state(self.state.unwrap())) {
                self.state = None;
                return None;
            }
            self.input.push(x);
        }
        return None;
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = None;
    }
}

#[derive(Debug, Clone)]
pub enum StdinInput {
    GraphicsResponse(GraphicsResponse),
    TerminalKey(TerminalKey),
    MouseEvent(MouseEvent),
}

#[derive(Debug, Clone, Copy)]
pub enum TerminalKey {
    UP,
    LEFT,
    RIGHT,
    DOWN,
    CTRLC,
    CTRLD,
    OTHER(u8),
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEventType {
    LCLICK,
    RCLICK,
    LRELEASE,
    RRELEASE,
    HOVER,
    HIGHLIGHT,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: usize,
    pub y: usize,
    pub event: MouseEventType,
}

/* A structure which extracts the Kitty graphics response in a lazy way */
#[derive(Debug, Clone)]
pub struct GraphicsResponse {
    source: String,
    loaded: bool,
    control: HashMap<String, String>,
    payload: String,
}

impl GraphicsResponse {
    pub fn new(source: &[u8]) -> Self {
        let source: &str = std::str::from_utf8(source).unwrap();
        let spl: Vec<&str> = source.split(';').collect();

        Self {
            source: spl.get(0).unwrap_or(&"").to_string(),
            loaded: false,
            control: HashMap::new(),
            payload: spl.get(1).unwrap_or(&"").to_string(),
        }
    }

    #[warn(dead_code)]
    fn load(&mut self) {
        let spl1: Split<char> = self.source.split(',');
        for kv in spl1 {
            let spl2: Vec<&str> = kv.split('=').collect();
            if spl2.len() != 2 {
                continue;
            }

            let _ = self
                .control
                .insert(spl2[0].to_string(), spl2[1].to_string());
        }

        self.loaded = true;
    }

    #[warn(dead_code)]
    pub fn control(&mut self) -> &HashMap<String, String> {
        if !self.loaded {
            self.load();
        }
        return &self.control;
    }

    pub fn payload(&self) -> &str {
        return self.payload.as_str();
    }
}

mod parsers {
    use super::{GraphicsResponse, MouseEvent, MouseEventType, StdinInput, TerminalKey};

    pub fn parse_ctrlc(_: &[u8]) -> StdinInput {
        StdinInput::TerminalKey(TerminalKey::CTRLC)
    }

    pub fn parse_ctrld(_: &[u8]) -> StdinInput {
        StdinInput::TerminalKey(TerminalKey::CTRLD)
    }

    pub fn parse_alphanumerical(x: &[u8]) -> StdinInput {
        StdinInput::TerminalKey(TerminalKey::OTHER(x[0]))
    }

    pub fn parse_udrl_key(x: &[u8]) -> StdinInput {
        const KEY_LU: [TerminalKey; 4] = [
            TerminalKey::UP,
            TerminalKey::DOWN,
            TerminalKey::RIGHT,
            TerminalKey::LEFT,
        ];
        StdinInput::TerminalKey(KEY_LU[*x.last().unwrap() as usize - 'A' as usize])
    }

    pub fn parse_mouse(x: &[u8]) -> StdinInput {
        let s: &str = std::str::from_utf8(&x[3..x.len() - 1]).unwrap();
        let data: Vec<&str> = s.split(';').collect();
        if data.len() != 3 {
            panic!("Invalid xTerm mouse command given: {:?}", x);
        }

        let press: bool = x[x.len() - 1] == b'M';
        let code: usize = data[0].parse::<usize>().expect(format!("{}", s).as_str());
        let x: usize = data[1].parse::<usize>().expect(format!("{}", s).as_str());
        let y: usize = data[2].parse::<usize>().expect(format!("{}", s).as_str());

        let res: StdinInput = match (code, press) {
            (0, true) => StdinInput::MouseEvent(MouseEvent {
                x: x,
                y: y,
                event: MouseEventType::LCLICK,
            }),
            (0, false) => StdinInput::MouseEvent(MouseEvent {
                x: x,
                y: y,
                event: MouseEventType::LRELEASE,
            }),
            (2, true) => StdinInput::MouseEvent(MouseEvent {
                x: x,
                y: y,
                event: MouseEventType::RCLICK
            }),
            (2, false) => StdinInput::MouseEvent(MouseEvent {
                x: x,
                y: y,
                event: MouseEventType::RRELEASE
            }),
            (35, true) | (35, false) => StdinInput::MouseEvent(MouseEvent {
                x: x,
                y: y,
                event: MouseEventType::HOVER,
            }),
            (64, true) | (64, false) => StdinInput::TerminalKey(TerminalKey::DOWN),
            (65, true) | (65, false) => StdinInput::TerminalKey(TerminalKey::UP),
            (66, true) | (66, false) => StdinInput::TerminalKey(TerminalKey::RIGHT),
            (67, true) | (67, false) => StdinInput::TerminalKey(TerminalKey::LEFT),
            _ => panic!(
                "Invalid xTerm mouse command given: Invalid mouse event {}",
                code
            ),
        };

        res
    }

    pub fn parse_graphics_response(x: &[u8]) -> StdinInput {
        StdinInput::GraphicsResponse(GraphicsResponse::new(&x[3..x.len() - 2]))
    }
}
