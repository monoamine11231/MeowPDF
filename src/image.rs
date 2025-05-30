use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    drivers::graphics::*, viewer::DisplayRect, CONFIG, IMAGE_PADDING, TERMINAL_SIZE,
};

use mupdf::Pixmap;

pub struct Image {
    id: usize,
    /* Stores the dimension of the zoomed in bitmap WITHOUT padding */
    dimensions: (i32, i32),
    data: Vec<u8>,
}

impl Image {
    pub fn new(pixmap: &Pixmap) -> Result<Self, String> {
        static ID: AtomicUsize = AtomicUsize::new(1);

        const PADDING_CLR: u8 = 0u8;
        let padding = *IMAGE_PADDING.get().unwrap();

        let mut data = Vec::with_capacity(
            (2 * padding + pixmap.width() as usize) * padding * 8
                + padding * 8
                + (pixmap.samples().len() / 3) * 4,
        );

        data.extend(std::iter::repeat_n(
            PADDING_CLR,
            (2 * padding + pixmap.width() as usize) * padding * 4,
        ));

        let row_iter = pixmap
            .samples()
            .chunks(pixmap.width() as usize * pixmap.n() as usize);
        for row in row_iter {
            data.extend(std::iter::repeat_n(PADDING_CLR, padding * 4));

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
            data.extend(std::iter::repeat_n(PADDING_CLR, padding * 4));
        }

        data.extend(std::iter::repeat_n(
            PADDING_CLR,
            (2 * padding + pixmap.width() as usize) * padding * 4,
        ));

        let image = Self {
            id: ID.load(Ordering::Acquire),
            dimensions: (pixmap.width() as i32, pixmap.height() as i32),
            data,
        };

        ID.store(ID.load(Ordering::Acquire) + 1, Ordering::Release);
        image.transfer()?;
        Ok(image)
    }

    #[allow(dead_code)]
    pub fn id(&self) -> usize {
        self.id
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    #[allow(dead_code)]
    pub fn check(&self) -> Result<(), String> {
        /* The first pixels should be invisible and therefore we have an easy if
         * the image still exists */
        terminal_graphics_display_image(self.id, 1, 1, (1, 1, 1, 1), 2, 2)?;
        Ok(())
    }

    pub fn display(&self, rect: DisplayRect) -> Result<bool, String> {
        /* `true` indicates that the image was actually displayed and was not
         * tried to be displayed outside of the viewpoint */

        let config = CONFIG.get().unwrap();
        let padding = *IMAGE_PADDING.get().unwrap();
        let render_precision = config.viewer.render_precision;

        let scale = rect.height as f64 / (self.dimensions.1 as f64 / render_precision);
        let render_precision_norm = render_precision / scale;

        let (pxpercol, pxperrow);
        let (col0, col1, row0, row1);
        let (padding_top, padding_bottom);
        let (padding_left, padding_right);
        let (cropx, cropy, cropw, croph);

        let terminal_size = TERMINAL_SIZE.get().unwrap().read().unwrap();

        pxpercol = terminal_size.width as f64 / terminal_size.columns as f64;
        pxperrow = terminal_size.height as f64 / terminal_size.rows as f64;

        if rect.x < 0 {
            col0 = 0.0f64;
        } else {
            col0 = rect.x as f64 / pxpercol;
        }
        col1 = (rect.x as f64 + rect.width as f64) / pxpercol;

        if rect.y < 0 {
            row0 = 0.0f64;
        } else {
            row0 = rect.y as f64 / pxperrow;
        }
        row1 = (rect.y as f64 + rect.height as f64) / pxperrow;

        /* Round up to the nearest whole col and row so that is guaranteed that the
         * the whole image is rendered without being shrinked down. `padding_*` values
         * tell how much of the image's invinsible padding should be included at each
         * side when displaying the image on an area of integer rows and column */
        padding_left = (col0 - col0.floor()) * pxpercol * render_precision_norm;
        padding_right = (col1.ceil() - col1) * pxpercol * render_precision_norm;
        padding_top = (row0 - row0.floor()) * pxperrow * render_precision_norm;
        padding_bottom = (row1.ceil() - row1) * pxperrow * render_precision_norm;

        if rect.x < 0 {
            cropx = (padding as f64 - rect.x as f64 * render_precision_norm) as usize;
            cropw = ((col1 * pxpercol * render_precision_norm) + padding_right) as usize;
        } else {
            cropx = (padding as f64 - padding_left) as usize;
            cropw = (padding_left + padding_right + self.dimensions.0 as f64) as usize;
        }

        if rect.y < 0 {
            cropy = (padding as f64 - rect.y as f64 * render_precision_norm) as usize;
            croph = ((row1 * pxperrow * render_precision_norm) + padding_bottom) as usize;
        } else {
            cropy = (padding as f64 - padding_top) as usize;
            croph = (padding_top + padding_bottom + self.dimensions.1 as f64) as usize;
        }

        /* If trying to display outside of terminal just return */
        if col1 < 0.0f64
            || row1 < 0.0f64
            || col0 > terminal_size.columns as f64
            || row0 > terminal_size.rows as f64
        {
            return Ok(false);
        }

        /* Do not forget that columns and rows are one-indexed in terminals */
        terminal_graphics_display_image(
            self.id,
            1 + col0.floor() as usize,
            1 + row0.floor() as usize,
            (cropx, cropy, cropw, croph),
            (col1.ceil() - col0.floor()) as usize,
            (row1.ceil() - row0.floor()) as usize,
        )?;

        Ok(true)
    }

    pub fn transfer(&self) -> Result<(), String> {
        let padding = *IMAGE_PADDING.get().unwrap();

        terminal_graphics_transfer_bitmap(
            self.id,
            self.dimensions.0 as usize + 2 * padding,
            self.dimensions.1 as usize + 2 * padding,
            self.data.as_slice(),
            true,
        )?;

        Ok(())
    }
}
