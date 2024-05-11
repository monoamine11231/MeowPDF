use crate::graphics::*;

use mupdf::Pixmap;
use nix::libc::winsize;

pub struct Image {
    id: usize,
    /* Stores the dimension of the zoomed in bitmap WITHOUT margin */
    dimensions: (i32, i32),
    precision: f64,
    margin: i32,
}

impl Image {
    pub fn new(pixmap: &Pixmap, precision: f64) -> Result<Self, String> {
        const MARGIN: usize = 400;
        const MARGIN_CLR: u8 = 0u8;

        let mut data: Vec<u8> = Vec::new();
        data.extend(
            std::iter::repeat(MARGIN_CLR)
                .take((2 * MARGIN + pixmap.width() as usize) * MARGIN * 4),
        );

        let row_iter = pixmap
            .samples()
            .chunks(pixmap.width() as usize * pixmap.n() as usize);
        for row in row_iter {
            data.extend_from_slice(&[MARGIN_CLR; MARGIN * 4]);

            /* If not RGBA extend by adding the alpha channel */
            if pixmap.n() == 3 {
                let pixel_iter = row.chunks(3);
                for pixel in pixel_iter {
                    data.extend_from_slice(pixel);
                    /* Add the alpha channel */
                    data.push(255);
                }
            } else {
                data.extend_from_slice(row);
            }
            data.extend_from_slice(&[MARGIN_CLR; MARGIN * 4]);
        }

        data.extend(
            std::iter::repeat(MARGIN_CLR)
                .take((2 * MARGIN + pixmap.width() as usize) * MARGIN * 4),
        );

        let id: usize = terminal_graphics_allocate_id()?;
        terminal_graphics_transfer_bitmap(
            id,
            pixmap.width() as usize + 2 * MARGIN,
            pixmap.height() as usize + 2 * MARGIN,
            data.as_slice(),
            true,
        )?;

        Ok(Self {
            id: id,
            dimensions: (pixmap.width() as i32, pixmap.height() as i32),
            precision: precision,
            margin: MARGIN as i32,
        })
    }

    pub fn display(
        &self,
        x: i32,
        y: i32,
        scale: f64,
        terminal_size: &winsize,
    ) -> Result<(), String> {
        let pxpercol: f64 = terminal_size.ws_xpixel as f64 / terminal_size.ws_col as f64;
        let pxperrow: f64 = terminal_size.ws_ypixel as f64 / terminal_size.ws_row as f64;

        let col0: f64;
        if x < 0 {
            col0 = 0.0f64;
        } else {
            col0 = x as f64 / pxpercol;
        }
        let col1: f64 =
            (x as f64 + self.dimensions.0 as f64 * scale / self.precision) / pxpercol;

        let row0: f64;
        if y < 0 {
            row0 = 0.0f64;
        } else {
            row0 = y as f64 / pxperrow;
        }
        let row1: f64 =
            (y as f64 + self.dimensions.1 as f64 * scale / self.precision) / pxperrow;

        let margin_left: f64 = (col0 - col0.floor()) * pxpercol * self.precision / scale;
        let margin_right: f64 = (col1.ceil() - col1) * pxpercol * self.precision / scale;
        let margin_top: f64 = (row0 - row0.floor()) * pxperrow * self.precision / scale;
        let margin_bottom: f64 = (row1.ceil() - row1) * pxperrow * self.precision / scale;

        let cropx: usize;
        if x < 0 {
            cropx = (self.margin as f64 - x as f64 * self.precision / scale) as usize;
        } else {
            cropx = (self.margin as f64 - margin_left) as usize;
        }

        let cropy: usize;
        if y < 0 {
            cropy = (self.margin as f64 - y as f64 * self.precision / scale) as usize;
        } else {
            cropy = (self.margin as f64 - margin_top) as usize;
        }

        let cropw: usize;
        if x < 0 {
            cropw = ((col1 * pxpercol * self.precision / scale) + margin_right) as usize;
        } else {
            cropw = (margin_left + margin_right + self.dimensions.0 as f64) as usize;
        }

        let croph: usize;
        if y < 0 {
            croph = ((row1 * pxperrow * self.precision / scale) + margin_bottom) as usize;
        } else {
            croph = (margin_top + margin_bottom + self.dimensions.1 as f64) as usize;
        }

        if col1 < 0.0f64
            || row1 < 0.0f64
            || col0 > terminal_size.ws_col as f64
            || row0 > terminal_size.ws_row as f64
        {
            return Ok(());
        }

        /* Do not forget that columns and rows are one-indexed in terminals */
        terminal_graphics_display_image(
            self.id,
            1 + col0.floor() as usize,
            1 + row0.floor() as usize,
            cropx,
            cropy,
            cropw,
            croph,
            (col1.ceil() - col0.floor()) as usize,
            (row1.ceil() - row0.floor()) as usize,
        )?;

        Ok(())
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        /* Remove the allocated graphics ID when dropping the state */
        let _ = terminal_graphics_deallocate_id(self.id);
    }
}
