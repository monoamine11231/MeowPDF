use crate::drivers::graphics::*;

use mupdf::Pixmap;
use nix::libc::winsize;

pub struct Image {
    id: usize,
    /* Stores the dimension of the zoomed in bitmap WITHOUT padding */
    dimensions: (i32, i32),
    precision: f64,
    padding: i32,
}

impl Image {
    pub fn new(pixmap: &Pixmap, precision: f64, padding: usize) -> Result<Self, String> {
        const PADDING_CLR: u8 = 0u8;

        let mut data: Vec<u8> = Vec::new();

        data.extend(
            std::iter::repeat(PADDING_CLR)
                .take((2 * padding + pixmap.width() as usize) * padding * 4),
        );

        let row_iter = pixmap
            .samples()
            .chunks(pixmap.width() as usize * pixmap.n() as usize);
        for row in row_iter {
            data.extend(std::iter::repeat(PADDING_CLR).take(padding * 4));

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
            data.extend(std::iter::repeat(PADDING_CLR).take(padding * 4));
        }

        data.extend(
            std::iter::repeat(PADDING_CLR)
                .take((2 * padding + pixmap.width() as usize) * padding * 4),
        );

        let id: usize = terminal_graphics_allocate_id()?;
        terminal_graphics_transfer_bitmap(
            id,
            pixmap.width() as usize + 2 * padding,
            pixmap.height() as usize + 2 * padding,
            data.as_slice(),
            true,
        )?;

        Ok(Self {
            id: id,
            dimensions: (pixmap.width() as i32, pixmap.height() as i32),
            precision: precision,
            padding: padding as i32,
        })
    }

    pub fn display(
        &self,
        x: i32,
        y: i32,
        scale: f64,
        terminal_size: &winsize,
    ) -> Result<(), String> {
        let (pxpercol, pxperrow): (f64, f64);
        let (col0, col1, row0, row1): (f64, f64, f64, f64);
        let (padding_top, padding_bottom): (f64, f64);
        let (padding_left, padding_right): (f64, f64);
        let (cropx, cropy, cropw, croph): (usize, usize, usize, usize);
        

        pxpercol = terminal_size.ws_xpixel as f64 / terminal_size.ws_col as f64;
        pxperrow = terminal_size.ws_ypixel as f64 / terminal_size.ws_row as f64;

        if x < 0 {
            col0 = 0.0f64;
        } else {
            col0 = x as f64 / pxpercol;
        }
        col1 = (x as f64 + self.dimensions.0 as f64 * scale / self.precision) / pxpercol;

        if y < 0 {
            row0 = 0.0f64;
        } else {
            row0 = y as f64 / pxperrow;
        }
        row1 = (y as f64 + self.dimensions.1 as f64 * scale / self.precision) / pxperrow;

        padding_left = (col0 - col0.floor()) * pxpercol * self.precision / scale;
        padding_right = (col1.ceil() - col1) * pxpercol * self.precision / scale;
        padding_top = (row0 - row0.floor()) * pxperrow * self.precision / scale;
        padding_bottom = (row1.ceil() - row1) * pxperrow * self.precision / scale;
        
        if x < 0 {
            cropx = (self.padding as f64 - x as f64 * self.precision / scale) as usize;
            cropw = ((col1 * pxpercol * self.precision / scale) + padding_right) as usize;
        } else {
            cropx = (self.padding as f64 - padding_left) as usize;
            cropw = (padding_left + padding_right + self.dimensions.0 as f64) as usize;
        }

        if y < 0 {
            cropy = (self.padding as f64 - y as f64 * self.precision / scale) as usize;
            croph =
                ((row1 * pxperrow * self.precision / scale) + padding_bottom) as usize;
        } else {
            cropy = (self.padding as f64 - padding_top) as usize;
            croph = (padding_top + padding_bottom + self.dimensions.1 as f64) as usize;
        }

        /* If trying to display outside of terminal just return */
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
