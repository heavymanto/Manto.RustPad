//! Come si scrive l'a-capo `\n` nei byte di un documento, e come lo si cerca.
//!
//! In una codifica compatibile con l'ASCII (UTF-8, code page) l'a-capo e' il byte 0x0A e `memchr` lo trova. In UTF-32
//! e' una unita' di 4 byte (`0A 00 00 00` o `00 00 00 0A`) e vale lo stesso ragionamento di UTF-16, con allineamento a 4.
//! In UTF-16 e' una unita' di 2 byte (`0A 00` in little endian, `00 0A` in big endian) e il solo byte 0x0A non basta: il byte
//! 0x0A compare anche dentro altri caratteri (U+010A e' `0A 01`, U+0A41 e' `41 0A`) e due unita' vicine possono
//! formare per caso la coppia dell'a-capo SFASATA di un byte (`41 0A` seguito da `00 01` contiene `0A 00` al byte 1).
//! Un a-capo e' quindi un'unita' ALLINEATA: la fetta che si esamina deve cominciare a una posizione multipla della
//! dimensione dell'unita' (e' sempre vero per le fette di un documento UTF-16 che parte da 0), e un byte finale che
//! non completa un'unita' non e' mai un a-capo.
//!
//! Le posizioni restituite sono relative all'inizio della fetta e puntano al PRIMO byte dell'unita'; la riga dopo un
//! a-capo comincia `unit()` byte piu' avanti.

use memchr::Memchr;

/// La forma dell'a-capo: una per famiglia di codifiche.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Newline {
    /// Il byte `0A` (UTF-8 e code page): la forma di sempre.
    #[default]
    Byte,
    /// L'unita' UTF-16 little endian: `0A 00`.
    Le16,
    /// L'unita' UTF-16 big endian: `00 0A`.
    Be16,
    /// L'unita' UTF-32 little endian: `0A 00 00 00`.
    Le32,
    /// L'unita' UTF-32 big endian: `00 00 00 0A`.
    Be32,
}

impl Newline {
    /// Quanti byte occupa un'unita' (e quindi l'allineamento): 1 per `Byte`, 2 per `Le16` e `Be16`, 4 per `Le32` e `Be32`.
    pub fn unit(self) -> usize {
        match self {
            Newline::Byte => 1,
            Newline::Le16 | Newline::Be16 => 2,
            Newline::Le32 | Newline::Be32 => 4,
        }
    }

    /// I byte che scrivono un a-capo.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Newline::Byte => b"\n",
            Newline::Le16 => b"\n\0",
            Newline::Be16 => b"\0\n",
            Newline::Le32 => b"\n\0\0\0",
            Newline::Be32 => b"\0\0\0\n",
        }
    }

    /// Vero se `unita` (esattamente `unit()` byte) e' un a-capo.
    pub fn is_newline(self, unita: &[u8]) -> bool {
        unita == self.bytes()
    }

    /// Le posizioni degli a-capo di `hay`, in ordine crescente (e anche all'indietro: `next_back`).
    pub fn positions(self, hay: &[u8]) -> Positions<'_> {
        Positions {
            nl: self,
            hay,
            inner: Memchr::new(b'\n', hay),
        }
    }

    /// Quanti a-capo ci sono in `hay`.
    pub fn count(self, hay: &[u8]) -> u64 {
        if self == Newline::Byte {
            return Memchr::new(b'\n', hay).count() as u64;
        }
        self.positions(hay).count() as u64
    }

    /// La posizione del primo a-capo di `hay`.
    pub fn find(self, hay: &[u8]) -> Option<usize> {
        self.positions(hay).next()
    }

    /// La posizione dell'ultimo a-capo di `hay`.
    pub fn rfind(self, hay: &[u8]) -> Option<usize> {
        self.positions(hay).next_back()
    }

    /// Vero se `hay` finisce con un a-capo: la sua lunghezza e' multipla di `unit()` e l'ultima unita' e' un a-capo.
    /// Un byte finale spaiato (UTF-16 di lunghezza dispari) fa rispondere falso.
    pub fn ends_with_newline(self, hay: &[u8]) -> bool {
        let u = self.unit();
        hay.len() >= u && hay.len().is_multiple_of(u) && self.is_newline(&hay[hay.len() - u..])
    }
}

/// Le posizioni degli a-capo di una fetta (vedi `Newline::positions`).
pub struct Positions<'a> {
    nl: Newline,
    hay: &'a [u8],
    inner: Memchr<'a>,
}

impl Positions<'_> {
    /// Se il byte 0x0A in `p` e' l'a-capo di questa forma, la posizione dell'unita'.
    fn unita(&self, p: usize) -> Option<usize> {
        match self.nl {
            Newline::Byte => Some(p),
            // `0A 00`: il 0x0A sta in posizione pari e il byte dopo (se c'e') e' 0
            Newline::Le16 => (p.is_multiple_of(2) && self.hay.get(p + 1) == Some(&0)).then_some(p),
            // `00 0A`: il 0x0A sta in posizione dispari e il byte prima e' 0
            Newline::Be16 => {
                if !p.is_multiple_of(2) && self.hay[p - 1] == 0 {
                    Some(p - 1)
                } else {
                    None
                }
            }
            // `0A 00 00 00`: il 0x0A sta in una posizione multipla di 4 e seguono tre byte 0
            Newline::Le32 => (p.is_multiple_of(4)
                && self.hay.get(p + 1..p + 4) == Some(&[0, 0, 0][..]))
            .then_some(p),
            // `00 00 00 0A`: il 0x0A sta in una posizione che e' 3 modulo 4 e precedono tre byte 0
            Newline::Be32 => {
                if p % 4 == 3 && self.hay[p - 3..p] == [0, 0, 0] {
                    Some(p - 3)
                } else {
                    None
                }
            }
        }
    }
}

impl Iterator for Positions<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            let p = self.inner.next()?;
            if let Some(u) = self.unita(p) {
                return Some(u);
            }
        }
    }
}

impl DoubleEndedIterator for Positions<'_> {
    fn next_back(&mut self) -> Option<usize> {
        loop {
            let p = self.inner.next_back()?;
            if let Some(u) = self.unita(p) {
                return Some(u);
            }
        }
    }
}
