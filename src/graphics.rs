use std::io::{stdin, stdout, Read, Stdin, Stdout, Write};


/* Should be executed only after uncooking the terminal. This method expects the
 * terminal that a non-blocking and unbuffered read from stdin is possible */
pub fn terminal_graphics_test_support() -> Result<(), String> {
    let mut handle1: Stdout = stdout();
    print!("\x1B_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1B\\\x1B[c");
    handle1.flush()
        .map_err(|x| format!("Could not flush stdout: {}", x))?;

    let mut buf: Vec<u8> = Vec::new();
    let mut handle2: Stdin = stdin();

    handle2.read_to_end(&mut buf)
        .map_err(|x| format!("Could not read from stdin: {}", x))?;

    let ind: Option<usize> = buf.as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);

    let res = match ind {
        Some(_) => Ok(()),
        None => Err("Terminal does not support Kitty graphics protocol.".to_string())
    };
    res
}

pub fn terminal_graphics_apc_success() -> Result<(), String> {
    let mut buf: Vec<u8> = Vec::new();

    let mut handle: Stdin = stdin();
    while buf.len() < 2 || buf.as_slice()[buf.len()-2..].to_vec()!=vec![b'\x1B', b'\\'] {
        let mut tmp: [u8;1] = [0];
        handle.read(&mut tmp)
            .map_err(|x| format!("Could not read from stdin: {}",x))?;
        buf.push(tmp[0]);
    }

    let ind: Option<usize> = buf.as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);
        
    let res = match ind {
        Some(_) => Ok(()),
        None => Err("Display or transfering of graphics to terminal has failed."
                        .to_string()),
    };
    res
}

