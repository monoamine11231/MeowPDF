use base64::{engine::general_purpose::STANDARD, Engine};
use regex::{Captures, Regex};
use std::{
    io::{stdin, stdout, Read, Stdin, Stdout, Write},
    iter::Peekable,
    ops::Deref,
    slice::Chunks,
    str,
    sync::OnceLock,
};

/* Should be executed only after uncooking the terminal. This method expects the
 * terminal that a non-blocking and unbuffered read from stdin is possible */
pub fn terminal_graphics_test_support() -> Result<(), String> {
    let mut handle1: Stdout = stdout();
    print!("\x1B_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1B\\\x1B[c");
    handle1
        .flush()
        .map_err(|x| format!("Could not flush stdout: {}", x))?;

    let mut buf: Vec<u8> = Vec::new();
    let mut handle2: Stdin = stdin();

    handle2
        .read_to_end(&mut buf)
        .map_err(|x| format!("Could not read from stdin: {}", x))?;

    let ind: Option<usize> = buf
        .as_slice()
        .windows(2)
        .position(|window| window == [b'O', b'K']);

    let res = match ind {
        Some(_) => Ok(()),
        None => Err("Terminal does not support Kitty graphics protocol.".to_string()),
    };
    res
}

pub fn terminal_graphics_apc_success() -> Result<(), String> {
    static REKITTYMSG: OnceLock<Result<Regex, String>> = OnceLock::new();
    let rekittymsg: Regex = REKITTYMSG
        .get_or_init(|| {
            return Regex::new(";([a-zA-Z]*)\x1B\\\\")
                .map_err(|x| format!("Could not compile regex expression: {}", x));
        })
        .clone()?;

    let mut handle: Stdin = stdin();
    let mut reply: String = String::new();
    handle
        .read_to_string(&mut reply)
        .map_err(|x| format!("Could not read from stdin: {}", x))?;
    let reply_captures: Captures<'_> = rekittymsg
        .captures(reply.as_str())
        .ok_or("Could not parse graphics responce from terminal".to_string())?;
    let reply_status: &str = reply_captures
        .get(1)
        .ok_or("Terminal replied with invalid responce to graphics query".to_string())?
        .as_str();

    if reply_status != "OK" {
        Err(format!(
            "Failed when trying to allocate graphics ID: {}",
            reply_status
        ))?;
    }
    
    Ok(())
}

pub fn terminal_graphics_allocate_id() -> Result<usize, String> {
    /* The new lazy static */
    static REKITTYMSG: OnceLock<Result<Regex, String>> = OnceLock::new();
    let rekittymsg: Regex = REKITTYMSG
        .get_or_init(|| {
            return Regex::new("i=(\\d+).*;([a-zA-Z]*)\x1B\\\\")
                .map_err(|x| format!("Could not compile regex expression: {}", x));
        })
        .clone()?;

    let mut handle1: Stdout = stdout();
    let mut handle2: Stdin = stdin();
    handle1
        .write("\x1B_Gf=24,I=1,t=d,v=1,s=1;AAAA\x1B\\".as_bytes())
        .map_err(|x| format!("Could not write to stdout: {}", x))?;
    handle1
        .flush()
        .map_err(|x| format!("Could not flush stdout: {}", x))?;

    let mut reply: String = String::new();
    handle2
        .read_to_string(&mut reply)
        .map_err(|x| format!("Could not read from stdin: {}", x))?;
    let reply_captures: Captures<'_> = rekittymsg
        .captures(reply.as_str())
        .ok_or("Could not parse graphics responce from terminal".to_string())?;

    let reply_status: &str = reply_captures
        .get(2)
        .ok_or("Terminal replied with invalid responce to graphics query".to_string())?
        .as_str();

    if reply_status != "OK" {
        Err(format!(
            "Failed when trying to allocate graphics ID: {}",
            reply_status
        ))?;
    }

    let reply_id: usize = reply_captures
        .get(1)
        .ok_or("Terminal replied with invalid responce to graphics query".to_string())?
        .as_str()
        .parse::<usize>()
        .map_err(|x| {
            return format!("Could not parse terminal generated graphics ID: {}", x);
        })?;

    Ok(reply_id)
}

pub fn terminal_graphics_deallocate_id(id: usize) -> Result<(), String> {
    let mut handle: Stdout = stdout();
    handle
        .write(format!("\x1B_Ga=d,d=I,i={};\x1B\\", id).as_bytes())
        .map_err(|x| format!("Could not write to stdout: {}", x))?;

    handle
        .flush()
        .map_err(|x| format!("Could not flush stdout: {}", x))?;
    Ok(())
}

pub fn terminal_graphics_transfer_rgba_bitmap(
    id: usize,
    width: usize,
    height: usize,
    data: &[u8],
) -> Result<(), String> {
    const CHUNK_SIZE: usize = 4096;

    let mut handle: Stdout = stdout();
    let encoded: String = STANDARD.encode(data);

    let mut chunks: Peekable<Chunks<u8>> = encoded.as_bytes().chunks(CHUNK_SIZE).peekable();

    /* First chunk with bitmap metadata */
    write!(
        handle,
        "\x1B_Gf=32,i={},s={},v={},m={};{}\x1B\\",
        id,
        width,
        height,
        (encoded.len() > CHUNK_SIZE) as i32,
        str::from_utf8(chunks.next().ok_or("No data provided")?)
            .map_err(|x| format!("Could not convert &[u8] to &str: {}", x))?
    )
    .map_err(|x| format!("Could not write to stdout: {}", x))?;

    while chunks.peek().is_some() {
        let data: &str = str::from_utf8(chunks.next().unwrap())
            .map_err(|x| format!("Could not convert &[u8] to &str: {}", x))?;

        write!(
            handle,
            "\x1B_Gm={};{}\x1B\\",
            chunks.peek().is_some() as i32,
            data
        )
        .map_err(|x| format!("Could not write to stdout: {}", x))?;
    }

    handle
        .flush()
        .map_err(|x| format!("Could not flush stdout: {}", x))?;

    terminal_graphics_apc_success()?;
    Ok(())
}
