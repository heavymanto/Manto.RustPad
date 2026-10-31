//! I caratteri per il testo: il monospace incorporato di egui non ha gli ideogrammi, quindi per le codifiche asiatiche
//! (e per qualunque file che li contenga) si carica un font di sistema in coda alla famiglia monospace. Se ne carica
//! UNO solo, scelto per la codifica del file (giapponese, cinese, coreano), per non pesare sulla memoria.

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily, FontTweak};

/// Di quanto si abbassano i glifi di un font asiatico, in frazione del corpo (vedi `definizioni`): i font di sistema
/// hanno la linea di base a altezze diverse, misurate guardando la schermata. Quelli che non sono in elenco restano come
/// sono.
fn spostamento_y(percorso: &str) -> f32 {
    const MISURATI: [(&str, f32); 1] = [("YuGoth", 0.24)];
    MISURATI
        .iter()
        .find(|(nome, _)| percorso.contains(nome))
        .map_or(0.0, |(_, f)| *f)
}
use mrp_core::encoding::Encoding;
use std::sync::Arc;

/// Per quale scrittura serve un font in piu'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scrittura {
    /// Solo latino (e quello che egui ha gia').
    Nessuna,
    Giapponese,
    CineseSemplificato,
    CineseTradizionale,
    Coreano,
}

/// La scrittura che una codifica richiede: le multibyte asiatiche. UTF-8 e UTF-16 possono avere di tutto: si prova il
/// giapponese (quello che copre piu' ideogrammi comuni con i kana).
pub fn scrittura_di(enc: Encoding) -> Scrittura {
    match enc.name() {
        "shift-jis" | "euc-jp" => Scrittura::Giapponese,
        "gbk" | "gb18030" => Scrittura::CineseSemplificato,
        "big5" => Scrittura::CineseTradizionale,
        "euc-kr" => Scrittura::Coreano,
        _ => Scrittura::Nessuna,
    }
}

/// I file dei font di sistema da provare per una scrittura, in ordine di preferenza (Windows, macOS, Linux).
pub fn candidati(s: Scrittura) -> &'static [&'static str] {
    match s {
        Scrittura::Nessuna => &[],
        Scrittura::Giapponese => &[
            "C:/Windows/Fonts/YuGothM.ttc",
            "C:/Windows/Fonts/msgothic.ttc",
            "C:/Windows/Fonts/meiryo.ttc",
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ],
        Scrittura::CineseSemplificato => &[
            "C:/Windows/Fonts/msyh.ttc",
            "C:/Windows/Fonts/simsun.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ],
        Scrittura::CineseTradizionale => &[
            "C:/Windows/Fonts/msjh.ttc",
            "C:/Windows/Fonts/mingliu.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ],
        Scrittura::Coreano => &[
            "C:/Windows/Fonts/malgun.ttf",
            "C:/Windows/Fonts/batang.ttc",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ],
    }
}

/// Vero se `c` e' un carattere che il font latino di egui non ha e che un font CJK ha: ideogrammi, kana, hangul,
/// forme a larghezza intera. Serve a caricare il font di ripiego anche per un file UTF-8 con testo asiatico.
pub fn e_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF | 0x3000..=0x30FF | 0x3100..=0x31FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF
        | 0x20000..=0x2FA1F)
}

/// La scrittura che serve per mostrare `c`, se e' un carattere asiatico: i kana e gli ideogrammi (il giapponese copre
/// tutti e due), l'hangul (il coreano). `None` per tutto il resto.
pub fn scrittura_del_carattere(c: char) -> Option<Scrittura> {
    match c as u32 {
        0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF => {
            Some(Scrittura::Coreano)
        }
        _ if e_cjk(c) => Some(Scrittura::Giapponese),
        _ => None,
    }
}

/// Le definizioni dei font: quelle di egui piu', per ogni scrittura richiesta, il primo font di sistema che si trova,
/// in coda ai monospace e ai proporzionali. Restituisce anche i percorsi caricati.
pub fn definizioni(scritture: &[Scrittura]) -> (FontDefinitions, Vec<&'static str>) {
    let mut def = FontDefinitions::default();
    let mut caricati: Vec<&'static str> = Vec::new();
    for (i, s) in scritture.iter().enumerate() {
        for percorso in candidati(*s) {
            // lo stesso file puo' servire due scritture (i Noto CJK): si carica una volta sola
            if caricati.contains(percorso) {
                break;
            }
            if let Ok(dati) = std::fs::read(percorso) {
                let nome = format!("cjk{i}");
                // i font asiatici di sistema hanno la linea di base piu' in alto di quella del monospace: si abbassano
                // (visivamente, senza toccare l'altezza della riga) per stare sulla stessa riga del testo latino
                let dati = FontData::from_owned(dati).tweak(FontTweak {
                    y_offset_factor: spostamento_y(percorso),
                    ..FontTweak::default()
                });
                def.font_data.insert(nome.clone(), Arc::new(dati));
                for famiglia in [FontFamily::Monospace, FontFamily::Proportional] {
                    def.families.entry(famiglia).or_default().push(nome.clone());
                }
                caricati.push(percorso);
                break;
            }
        }
    }
    (def, caricati)
}

/// Installa nel contesto i font per le scritture `scritture`.
pub fn installa(ctx: &Context, scritture: &[Scrittura]) {
    let (def, _) = definizioni(scritture);
    ctx.set_fonts(def);
}
