//! L'icona del progetto per la finestra (barra del titolo e barra delle applicazioni): lo stesso PNG che `build.rs` mette dentro
//! l'eseguibile per Esplora risorse (`rust/assets/icon.png`).

use eframe::egui::IconData;
use std::io::Cursor;

/// Il file PNG dell'icona.
pub const PNG: &[u8] = include_bytes!("../../../assets/icon.png");

/// L'icona come la vuole la finestra: punti RGBA a 8 bit. `None` se il PNG non e' RGBA a 8 bit (non succede: lo prova un collaudo).
pub fn icona_finestra() -> Option<IconData> {
    let mut lettore = png::Decoder::new(Cursor::new(PNG)).read_info().ok()?;
    let mut punti = vec![0; lettore.output_buffer_size()?];
    let quadro = lettore.next_frame(&mut punti).ok()?;
    if quadro.color_type != png::ColorType::Rgba || quadro.bit_depth != png::BitDepth::Eight {
        return None;
    }
    punti.truncate(quadro.buffer_size());
    Some(IconData {
        rgba: punti,
        width: quadro.width,
        height: quadro.height,
    })
}
