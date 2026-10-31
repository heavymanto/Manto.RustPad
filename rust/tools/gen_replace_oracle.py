"""Genera crates/mrp-core/tests/replace_oracle_data.rs: casi (pattern, sostituzione Rust, testo, risultato atteso) il
cui risultato lo calcola Python (`re.sub` riga per riga, come le regole per riga di mrp_core::pattern). Python e' un
oracolo indipendente dal crate `regex`: dove i due motori differiscono il caso non e' in elenco. Si usa:
    python tools/gen_replace_oracle.py > crates/mrp-core/tests/replace_oracle_data.rs
"""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", newline="\n")

TESTO_A = "Mario Rossi <mario@rossi.com>\nluigi.verdi@verdi.it e anna@bianchi.com\nnessuna mail qui\n  rientrata  \nfine"
TESTO_B = "colour color COLOUR\nthe cat sat on the concatenate cat\nxx xxx xxxx x\nabab ababab ba\n"
TESTO_C = "tel 0123 456 7890, cap 20100\nv1.2.3 v10.20.30\ncittà è caffè\nÈ la città\n"

# (pattern per Rust e per Python, sostituzione Python, sostituzione Rust, testo)
CASI = [
    (r"\d+", "N", "N", TESTO_C),
    (r"\d+", "<\\g<0>>", "<${0}>", TESTO_C),
    (r"(\w+)@(\w+)\.(com|it)", "\\g<2>:\\g<1>", "${2}:${1}", TESTO_A),
    (r"^\s+", "", "", TESTO_A),
    (r"\s+$", "", "", TESTO_A),
    (r"[aeiou]", "_", "_", TESTO_B),
    (r"colou?r", "C", "C", TESTO_B),
    (r"(?i)colou?r", "C", "C", TESTO_B),
    (r"\bcat\b", "dog", "dog", TESTO_B),
    (r"x{2,3}", "Y", "Y", TESTO_B),
    (r"(ab)+", "[\\g<0>]", "[${0}]", TESTO_B),
    (r"(a|b)+", "-", "-", TESTO_B),
    (r"x.*?x", "Z", "Z", TESTO_B),
    (r"\w+", "w", "w", TESTO_C),
    (r"(?i)è", "e", "e", TESTO_C),
    (r"v(\d+)\.(\d+)\.(\d+)", "\\g<3>.\\g<2>.\\g<1>", "${3}.${2}.${1}", TESTO_C),
    (r"[^ ]+@[^ ]+", "MAIL", "MAIL", TESTO_A),
    (r"\.", "!", "!", TESTO_C),
    (r"(?P<a>\d)(?P<b>\d)", "\\g<b>\\g<a>", "${b}${a}", TESTO_C),
    (r"a|b", "", "", TESTO_B),
]


def rs(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t') + '"'


print("// Generato da tools/gen_replace_oracle.py: non si modifica a mano.")
print("pub const CASI: &[(&str, &str, &str, &str)] = &[")
for pat, py_rep, rs_rep, testo in CASI:
    rx = re.compile(pat)
    righe = testo.split("\n")
    for r in righe:
        # un'espressione che puo' combaciare con il vuoto non e' ammessa nel corpus (il motore Rust scarta il vuoto)
        assert rx.fullmatch("") is None, pat
    atteso = "\n".join(rx.sub(py_rep, r) for r in righe)
    print(f"    ({rs(pat)}, {rs(rs_rep)}, {rs(testo)}, {rs(atteso)}),")
print("];")
