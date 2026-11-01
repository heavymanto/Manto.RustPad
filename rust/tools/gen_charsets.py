#!/usr/bin/env python3
"""Genera crates/mrp-core/src/charsets.rs: le tabelle delle code page a byte singolo (la meta' alta, 0x80-0xFF).

Le tabelle vengono dai codec di Python (fonte indipendente dal codice Rust). Un byte che la code page non definisce
(per esempio 0x81 in windows-1252) vale U+FFFD: nel programma e' un byte non valido, come in UTF-8.

Uso (dalla cartella rust/):  python tools/gen_charsets.py && cargo fmt --all
(il file generato va formattato con rustfmt, come tutto il resto)
"""
import os
import sys

# (nome canonico, codec di Python, alias oltre al nome canonico; il nome si confronta senza maiuscole e con '_' = '-')
CHARSETS = [
    ("windows-1250", "cp1250", ["cp1250", "1250"]),
    ("windows-1251", "cp1251", ["cp1251", "1251"]),
    ("windows-1252", "cp1252", ["cp1252", "1252", "ansi"]),
    ("windows-1253", "cp1253", ["cp1253", "1253"]),
    ("windows-1254", "cp1254", ["cp1254", "1254"]),
    ("windows-1255", "cp1255", ["cp1255", "1255"]),
    ("windows-1256", "cp1256", ["cp1256", "1256"]),
    ("windows-1257", "cp1257", ["cp1257", "1257"]),
    ("windows-1258", "cp1258", ["cp1258", "1258"]),
    ("iso-8859-1", "iso8859-1", ["iso8859-1", "latin1", "latin-1", "l1"]),
    ("iso-8859-2", "iso8859-2", ["iso8859-2", "latin2", "latin-2", "l2"]),
    ("iso-8859-3", "iso8859-3", ["iso8859-3", "latin3", "latin-3", "l3"]),
    ("iso-8859-4", "iso8859-4", ["iso8859-4", "latin4", "latin-4", "l4"]),
    ("iso-8859-5", "iso8859-5", ["iso8859-5", "cyrillic"]),
    ("iso-8859-6", "iso8859-6", ["iso8859-6", "arabic"]),
    ("iso-8859-7", "iso8859-7", ["iso8859-7", "greek"]),
    ("iso-8859-8", "iso8859-8", ["iso8859-8", "hebrew"]),
    ("iso-8859-9", "iso8859-9", ["iso8859-9", "latin5", "latin-5", "l5"]),
    ("iso-8859-10", "iso8859-10", ["iso8859-10", "latin6", "latin-6", "l6"]),
    ("iso-8859-11", "iso8859-11", ["iso8859-11", "thai"]),
    ("iso-8859-13", "iso8859-13", ["iso8859-13", "latin7", "latin-7", "l7"]),
    ("iso-8859-14", "iso8859-14", ["iso8859-14", "latin8", "latin-8", "l8"]),
    ("iso-8859-15", "iso8859-15", ["iso8859-15", "latin9", "latin-9", "l9"]),
    ("iso-8859-16", "iso8859-16", ["iso8859-16", "latin10", "latin-10", "l10"]),
    ("ibm437", "cp437", ["cp437", "437"]),
    ("ibm737", "cp737", ["cp737", "737"]),
    ("ibm850", "cp850", ["cp850", "850"]),
    ("ibm852", "cp852", ["cp852", "852"]),
    ("ibm858", "cp858", ["cp858", "858"]),
    ("ibm866", "cp866", ["cp866", "866"]),
    ("koi8-r", "koi8-r", ["koi8r"]),
    ("koi8-u", "koi8-u", ["koi8u"]),
    ("macintosh", "mac_roman", ["mac", "macroman", "mac-roman", "x-mac-roman"]),
    # aggiunte (E25): in coda, cosi' gli indici delle code page di prima non cambiano.
    # mac-greek (4.3) e' in coda e usa OVERRIDES: vedi sotto.
    # Mancano di proposito cp864 (la sua meta' bassa non e' ASCII: 0x25 e' '%' arabo) e le EBCDIC (cp037...: '\n' non e' 0x0A)
    ("us-ascii", "ascii", ["ascii", "iso646-us"]),  # nessun byte sopra 0x7F: tutta la meta' alta vale U+FFFD
    ("windows-874", "cp874", ["cp874", "874", "tis-620", "tis620"]),
    ("ibm775", "cp775", ["cp775", "775"]),
    ("ibm855", "cp855", ["cp855", "855"]),
    ("ibm857", "cp857", ["cp857", "857"]),
    ("ibm860", "cp860", ["cp860", "860"]),
    ("ibm861", "cp861", ["cp861", "861"]),
    ("ibm862", "cp862", ["cp862", "862"]),
    ("ibm863", "cp863", ["cp863", "863"]),
    ("ibm865", "cp865", ["cp865", "865"]),
    ("ibm869", "cp869", ["cp869", "869"]),
    ("ibm720", "cp720", ["cp720", "720"]),
    ("ibm1125", "cp1125", ["cp1125", "1125"]),
    ("mac-cyrillic", "mac_cyrillic", ["maccyrillic", "x-mac-cyrillic"]),
    ("mac-centraleurope", "mac_latin2", ["maccentraleurope", "x-mac-ce", "x-mac-centraleurroman"]),
    ("mac-iceland", "mac_iceland", ["maciceland", "x-mac-icelandic"]),
    ("mac-turkish", "mac_turkish", ["macturkish", "x-mac-turkish"]),
    ("mac-croatian", "mac_croatian", ["maccroatian", "x-mac-croatian"]),
    ("mac-romanian", "mac_romanian", ["macromanian", "x-mac-romanian"]),
    ("mac-greek", "mac_greek", ["macgreek", "x-mac-greek"]),
]

# Mac Greek: tre fonti indipendenti si dividono su tre byte. Python (che segue la revisione piu' recente di Apple, con l'euro)
# vuole 0x9C = U+20AC, 0xAF = U+00B7, 0xFF = U+00AD; .NET (CP 10006) e GNU libiconv (MACGREEK) sono d'accordo fra loro su
# 0x9C = U+00AD (trattino debole) e 0xAF = U+0387 (ano teleia), e su 0xFF non lo sono (.NET: U+F8A0 dell'area privata di
# Apple, libiconv: non definito). Si segue l'accordo di due fonti su due byte e si lascia 0xFF non definito (un byte non valido,
# come in UTF-8: l'alternativa e' un carattere privato che nessun font ha). None = non definito.
OVERRIDES = {
    "mac-greek": {0x9C: "\u00ad", 0xAF: "\u0387", 0xFF: None},
}

# un nome o alias deve indicare una sola code page (il confronto e' senza maiuscole e con '_' = '-')
_visti = {}
for _n, _c, _a in CHARSETS:
    for _nome in [_n] + _a:
        assert _nome not in _visti, (_nome, _n, _visti.get(_nome))
        _visti[_nome] = _n


def table(codec, nome=""):
    # la meta' bassa deve essere ASCII: il resto del programma (righe, ricerca) conta su '\n' = 0x0A
    for b in range(0x80):
        assert bytes([b]).decode(codec) == chr(b), (codec, b)
    out = []
    sovrascritti = OVERRIDES.get(nome, {})
    for b in range(0x80, 0x100):
        if b in sovrascritti:
            out.append(sovrascritti[b] if sovrascritti[b] is not None else "\ufffd")
            continue
        try:
            s = bytes([b]).decode(codec)
            assert len(s) == 1, (codec, b, s)
            out.append(s)
        except UnicodeDecodeError:
            # Windows-125x: un byte 0x80-0x9F che la code page non definisce vale il controllo C1 con lo stesso numero
            # (come fanno Windows e i browser, WHATWG): scritto e riletto lascia il byte com'era, invece di perderlo
            if nome.startswith("windows-125") and b < 0xA0:
                out.append(chr(b))
            else:
                out.append("�")
    # ogni carattere definito vale per un solo byte: serve perche' scrivere un carattere sia univoco
    definiti = [c for c in out if c != "�"]
    assert len(definiti) == len(set(definiti)), codec
    return out


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    dest = os.path.join(here, "..", "crates", "mrp-core", "src", "charsets.rs")
    lines = [
        "//! Le code page a byte singolo (meta' alta 0x80-0xFF). FILE GENERATO da tools/gen_charsets.py (codec di Python):",
        "//! non si modifica a mano. Un byte non definito dalla code page vale U+FFFD.",
        "",
        "/// Una code page a byte singolo: i byte 0x00-0x7F sono ASCII, `table[b - 0x80]` e' il carattere del byte `b`.",
        "pub struct Charset {",
        "    /// Nome canonico (minuscolo).",
        "    pub name: &'static str,",
        "    /// Altri nomi accettati (minuscoli).",
        "    pub aliases: &'static [&'static str],",
        "    /// I caratteri dei byte 0x80-0xFF (U+FFFD: byte non definito).",
        "    pub table: [char; 128],",
        "}",
        "",
        "/// Tutte le code page a byte singolo note.",
        "pub static CHARSETS: [Charset; %d] = [" % len(CHARSETS),
    ]
    for name, codec, aliases in CHARSETS:
        t = table(codec, name)
        lines.append("    Charset {")
        lines.append('        name: "%s",' % name)
        lines.append("        aliases: &[%s]," % ", ".join('"%s"' % a for a in aliases))
        lines.append("        table: [")
        for i in range(0, 128, 8):
            lines.append("            " + " ".join("'\\u{%x}'," % ord(c) for c in t[i:i + 8]))
        lines.append("        ],")
        lines.append("    },")
    lines.append("];")
    with open(dest, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("scritto", os.path.normpath(dest), len(CHARSETS), "code page")


if __name__ == "__main__":
    sys.exit(main())
