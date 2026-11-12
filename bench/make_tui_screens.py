"""Generates the TUI screenshots for the documentation: runs mrp-tui in --dump mode (the screen
as text, deterministic, no terminal needed) and renders each dump as a PNG with PIL — a dark
background, a monospace font, the terminal look. Every image is the same 120x30 grid, the same
Dark theme, the same test file: what changes is only what the keys show.

    cargo build --release -p mrp-tui        (from rust/)
    python -I bench/make_tui_screens.py     (from the repository root)
"""
import gzip
import os
import subprocess
import sys
import tempfile

RADICE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(RADICE, 'rust', 'target', 'release', 'mrp-tui' + ('.exe' if os.name == 'nt' else ''))
DEST = os.path.join(RADICE, 'docs', 'img', 'tui')
SETTINGS = r'C:\temp\mrp-manuale-gui\settings.json' if os.name == 'nt' else '/tmp/mrp-manuale-gui/settings.json'

if not os.path.exists(EXE):
    sys.exit('build first: cargo build --release -p mrp-tui (from rust/); not found: ' + EXE)

COLONNE = 120
RIGHE = 30

# (nome, chiavi --keys, descrizione breve per il log)
SCATTI = [
    ('n01-apre', '', 'il file aperto nel terminale'),
    ('n04-vai', 'F5', 'il prompt Vai a'),
    ('n05-salti', 'Ctrl+B', 'la barra dei salti'),
    ('m02-modi', 'Alt+M', 'il menu dei modi'),
    ('t01-formato', 'Alt+O', 'la finestra del formato'),
    ('t03-intervallo', 'Alt+I', 'la finestra del tempo'),
    ('t04-timeline', 'Alt+B', 'la timeline accanto al testo'),
    ('t07-colori', '', 'i colori dei log'),
    ('f01-cerca', 'Ctrl+F', 'il prompt Cerca'),
    ('f02-sostituisci', 'F4', 'la finestra Sostituisci'),
    ('f04-lunghezze', 'Alt+H', 'il check dei tracciati'),
    ('e01-codifica', 'F9', 'il menu delle codifiche'),
    ('h-aiuto', 'F1', 'la tabella dei tasti'),
]


def scompatta(nome, cartella):
    percorso = os.path.join(cartella, nome + '.log')
    if not os.path.exists(percorso):
        with gzip.open(os.path.join(RADICE, 'bench', 'samples', nome + '.log.gz'), 'rb') as z:
            with open(percorso, 'wb') as f:
                f.write(z.read())
    return percorso


def dump(chiavi, file):
    """One --dump run: the screen as a list of text lines."""
    cmd = [EXE, '--dump', f'{COLONNE}x{RIGHE}', '--theme', 'dark', '--settings', SETTINGS, file]
    if chiavi:
        cmd += ['--keys', chiavi]
    r = subprocess.run(cmd, capture_output=True, timeout=30)
    if r.returncode != 0:
        sys.exit('mrp-tui failed: ' + r.stderr.decode('utf-8', errors='replace'))
    testo = r.stdout.decode('utf-8', errors='replace')
    return testo.splitlines()


def render(righe, percorso):
    """Renders the text lines as a PNG that looks like a terminal."""
    from PIL import Image, ImageDraw, ImageFont

    # un font monospace: Consolas su Windows, fallback a quello di PIL
    for nome in ['consola.ttf', 'lucon.ttf', 'cour.ttf']:
        p = os.path.join(os.environ.get('WINDIR', r'C:\Windows'), 'Fonts', nome)
        if os.path.exists(p):
            font = ImageFont.truetype(p, 15)
            break
    else:
        font = ImageFont.load_default()

    # la cella di un carattere
    prova = Image.new('1', (1, 1))
    disegno = ImageDraw.Draw(prova)
    bb = disegno.textbbox((0, 0), 'M', font=font)
    cella_w = bb[2] - bb[0]
    cella_h = 20  # le righe del terminale sono alte

    img = Image.new('RGB', (cella_w * COLONNE, cella_h * len(righe)), (30, 30, 30))
    disegno = ImageDraw.Draw(img)

    for i, riga in enumerate(righe):
        y = i * cella_h
        # il testo principale: grigio chiaro
        disegno.text((2, y), riga, fill=(212, 212, 212), font=font)

    img.save(percorso)


def main():
    os.makedirs(DEST, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mrp_tui_') as cartella:
        file = scompatta('epoch', cartella)
        for nome, chiavi, cosa in SCATTI:
            righe = dump(chiavi, file)
            percorso = os.path.join(DEST, nome + '.png')
            render(righe, percorso)
            print('scatto tui:', nome, '-', cosa)
    print('done: docs/img/tui/*.png')


if __name__ == '__main__':
    main()
