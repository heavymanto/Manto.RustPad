"""Regenerates the documentation images (the manual screenshots and the README GIF) with the
program's own developer hooks: everything deterministic, no screen, no clicking.

    cargo build --release -p mrp-gui        (from rust/)
    python -I bench/make_docs_images.py     (from the repository root)

The log files come from bench/samples (always the same ones: mrp-loggen writes them). Every window
is the factory one (1000x700, Dark theme, English): what changes between two runs is only what the
program shows. The GIF is a film of the scroll (MRPG_FILM): one frame every MRPG_FILM_MS with a
wheel down in between, assembled with PIL.
"""
import gzip
import os
import subprocess
import sys
import tempfile

RADICE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(RADICE, 'rust', 'target', 'release', 'mrp-gui' + ('.exe' if os.name == 'nt' else ''))
IMG = os.path.join(RADICE, 'docs', 'img')
MANUALE = os.path.join(IMG, 'manual')

if not os.path.exists(EXE):
    sys.exit('build first: cargo build --release -p mrp-gui (from rust/); not found: ' + EXE)

# The manual screenshots: (name, MRPG_AZIONE list — None for the settings window, [] for none,
# extra files, MRPG_ATTESA_MS — the wait before the shot, for what arrives late)
SCATTI = [
    ('n01-apre', [], [], 0),
    ('n02-schede', [], ['bursts'], 0),
    ('n04-vai', ['goto'], [], 0),
    ('n05-salti', ['salti'], [], 0),
    ('m02-modi', ['modi'], [], 0),
    ('m08-tail', ['tail'], [], 0),
    ('t01-formato', ['formato'], [], 0),
    ('t03-intervallo', ['calendario'], [], 0),
    ('t04-timeline', ['timeline'], [], 1500),
    ('t07-colori', [], [], 0),
    ('f01-cerca', ['find'], [], 0),
    ('f02-sostituisci', ['replace'], [], 0),
    ('f03-filtri', ['contestuale'], [], 0),
    ('f04-lunghezze', ['lunghezze'], [], 800),
    ('e01-codifica', ['codifica'], [], 0),
    ('h-aiuto', ['aiuto'], [], 0),
    ('h-impostazioni', None, [], 0),
]


def scompatta(nome, cartella):
    """The plain log from bench/samples (deterministic: the same bytes every time)."""
    percorso = os.path.join(cartella, nome + '.log')
    if not os.path.exists(percorso):
        with gzip.open(os.path.join(RADICE, 'bench', 'samples', nome + '.log.gz'), 'rb') as z:
            with open(percorso, 'wb') as f:
                f.write(z.read())
    return percorso


def ambiente(**variabili):
    """A clean environment: only the MRPG_* hooks asked for, nothing left over from the caller."""
    env = {k: v for k, v in os.environ.items() if not k.startswith('MRPG_')}
    env.update(variabili)
    return env


def scatta(nome, azioni, extra, attesa, cartella):
    """One screenshot: MRPG_SCREENSHOT saves it and exits after a few frames."""
    file = [scompatta('epoch', cartella)] + [scompatta(e, cartella) for e in extra]
    env = ambiente(MRPG_SCREENSHOT=os.path.join(MANUALE, nome + '.png'))
    if attesa:
        env['MRPG_ATTESA_MS'] = str(attesa)
    if azioni is None:
        env['MRPG_IMPOSTAZIONI'] = '1'
    elif azioni:
        env['MRPG_AZIONE'] = ','.join(azioni)
    subprocess.run([EXE, '--theme', 'dark'] + file, env=env, check=True, timeout=120)
    print('scatto:', nome)


def filmato(cartella):
    """The README GIF: MRPG_FILM shoots the frames of a scroll, PIL assembles them."""
    from PIL import Image
    quadri = tempfile.mkdtemp(prefix='mrp_film_')
    env = ambiente(
        MRPG_FILM=quadri,
        MRPG_FILM_N='30',
        MRPG_FILM_MS='140',
    )
    subprocess.run(
        [EXE, '--theme', 'dark', scompatta('epoch', cartella)],
        env=env,
        check=True,
        timeout=180,
    )
    nomi = sorted(n for n in os.listdir(quadri) if n.endswith('.png'))
    if len(nomi) < 2:
        sys.exit('the film has {} frames: {}'.format(len(nomi), quadri))
    frames = []
    for n in nomi:
        with Image.open(os.path.join(quadri, n)) as imm:
            imm = imm.convert('RGB')
            # il README non ha bisogno dei mille pixel: 560 di larghezza bastano
            larga = 560
            alta = round(imm.height * larga / imm.width)
            frames.append(imm.resize((larga, alta), Image.LANCZOS))
    # una palette sola per tutti i fotogrammi, senza dithering: le aree piatte compressano
    base = frames[0].quantize(colors=32)
    frames = [base] + [f.quantize(palette=base, dither=Image.Dither.NONE) for f in frames[1:]]
    frames[0].save(
        os.path.join(IMG, 'gui-demo.gif'),
        save_all=True,
        append_images=frames[1:],
        duration=140,
        loop=0,
        optimize=True,
    )
    print('gif: {} frames'.format(len(frames)))


def main():
    os.makedirs(MANUALE, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mrp_docs_') as cartella:
        if '--solo-gif' not in sys.argv:
            for nome, azioni, extra, attesa in SCATTI:
                scatta(nome, azioni, extra, attesa, cartella)
        filmato(cartella)
    print('done: docs/img/manual/*.png and docs/img/gui-demo.gif')


if __name__ == '__main__':
    main()
