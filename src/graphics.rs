use std::io::{stdin, stdout, Read, Stdin, Stdout, Write};


pub fn terminal_graphics_test_support() -> Result<(), &'static str> {
    let mut handle1: Stdout = stdout();
    print!("\x1B_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1B\\\x1B[c");
    handle1.flush()
        .expect("Could not flush stdout.");

    let mut buf: Vec<u8> = Vec::new();
    let mut handle2: Stdin = stdin();

    handle2.read_to_end(&mut buf)
        .expect("Could not read from stdin.");

    let ind: Option<usize> = buf.as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);

    let res = match ind {
        Some(_) => Ok(()),
        None => Err("Terminal does not support Kitty graphics protocol.")
    };
    res
}

pub fn terminal_graphics_apc_success() -> Result<(), &'static str> {
    let mut buf: Vec<u8> = Vec::new();

    let mut handle: Stdin = stdin();
    while buf.len() < 2 || buf.as_slice()[buf.len()-2..].to_vec()!=vec![b'\x1B', b'\\'] {
        let mut tmp: [u8;1] = [0];
        handle.read(&mut tmp)
            .expect("Could not read from stdin");
        buf.push(tmp[0]);
    }

    let ind: Option<usize> = buf.as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);
        
    let res = match ind {
        Some(_) => Ok(()),
        None => Err("Display or transfering of graphics to terminal has failed."),
    };
    res
}