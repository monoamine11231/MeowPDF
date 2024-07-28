use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs::File,
    io::{stdout, StdoutLock, Write},
    path::PathBuf,
    time::Duration,
};

use crate::{RECEIVER_GR, SOFTWARE_ID};

/* Should be executed only after uncooking the terminal. This method expects the
 * terminal that a non-blocking and unbuffered read from stdin is possible */
pub fn terminal_graphics_test_support() -> Result<(), String> {
    let mut handle1: StdoutLock = stdout().lock();
    handle1
        .write(b"\x1B_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1B\\")
        .unwrap();
    handle1.flush().unwrap();

    /* Timeout since we don't really know yet if the kitty graphics protocol
     * is supported or not */
    let response = RECEIVER_GR
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .recv_timeout(Duration::from_millis(100))
        .map_err(|x| {
            format!("Could not receive from Graphics Response channel: {}", x)
        })?;

    if !response.payload().contains("OK") {
        Err(format!(
            "Terminal responded with failed graphics response: {}",
            response.payload()
        ))?;
    }
    Ok(())
}

#[allow(dead_code)]
pub fn terminal_graphics_deallocate_id(id: usize) -> Result<(), String> {
    let mut handle: StdoutLock = stdout().lock();
    write!(handle, "\x1B_Ga=d,d=I,i={};\x1B\\", id).unwrap();

    handle.flush().unwrap();

    Ok(())
}

pub fn terminal_graphics_transfer_bitmap(
    id: usize,
    width: usize,
    height: usize,
    data: &[u8],
    alpha: bool,
) -> Result<(), String> {
    let mut tmp_file_path: PathBuf = std::env::temp_dir();
    tmp_file_path.push(format!(
        "tty-graphics-protocol-{}",
        SOFTWARE_ID.get().unwrap()
    ));

    {
        let mut tmp_file: File = File::create(tmp_file_path.as_path()).unwrap();
        tmp_file.write(data).unwrap();
    }

    let mut handle: StdoutLock = stdout().lock();

    /* First chunk with bitmap metadata */
    write!(
        handle,
        "\x1B_Gq=2,f={},i={},s={},v={},t=t;{}\x1B\\",
        if alpha { 32 } else { 24 },
        id,
        width,
        height,
        STANDARD.encode(tmp_file_path.to_str().unwrap())
    )
    .unwrap();

    handle.flush().unwrap();

    Ok(())
}

pub fn terminal_graphics_display_image(
    id: usize,
    col: usize,
    row: usize,

    x: usize,
    y: usize,
    w: usize,
    h: usize,

    c: usize,
    r: usize,
) -> Result<(), String> {
    let mut handle: StdoutLock = stdout().lock();

    write!(handle, "\x1B[s\x1B[{};{}H", row, col).unwrap();

    /* Z-index < -1,073,741,824 will make the images to be drawn behind
     * cells with colored background */
    write!(
        handle,
        "\x1B_Gz=-1073741825,a=p,C=1,i={},x={},y={},w={},h={},c={},r={};\x1B\\",
        id, x, y, w, h, c, r
    )
    .unwrap();

    handle.write(b"\x1B[u").unwrap();

    handle
        .flush()
        .map_err(|x: std::io::Error| format!("Could not flush stdout: {}", x))?;

    Ok(())
}
