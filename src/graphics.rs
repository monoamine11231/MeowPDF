use std::io::{stdin,Stdin,Read};


pub fn terminal_graphics_apc_success() -> Result<(), &'static str> {
    let mut buf: Vec<u8> = Vec::new();

    let mut handle: Stdin = stdin();
    while buf.len() < 2 || buf.as_slice()[buf.len()-2..].to_vec()!=vec![b'\x1B', b'\\'] {
        let mut tmp: [u8;1] = [0];
        handle.read(&mut tmp)
            .expect("ERROR: Could not read from stdin");
        buf.push(tmp[0]);
    }

    let ind: Option<usize> = buf.as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);
        
    let res = match ind {
        Some(_x) => Ok(()),
        None => Err("ERROR: Display or transfering of graphics to terminal has failed."),
    };
    res
}