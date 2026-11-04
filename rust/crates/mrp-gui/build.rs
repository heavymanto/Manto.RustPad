//! Mette l'icona del progetto (`rust/assets/icon.png`) dentro mrp-gui.exe: vedi `build_support/icona.rs`.

include!("../../build_support/icona.rs");

fn main() {
    incorpora_icona("../../assets/icon.png");
}
