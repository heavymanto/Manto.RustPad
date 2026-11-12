"""Modifies both manuals: removes 'Cosa fa'/'What it does' from all cards, adds TUI screenshots
where GUI screenshots exist, adds navigation links at the top. Run once, then review the diff."""
import os
import re

RADICE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TUI_DIR = os.path.join(RADICE, 'docs', 'img', 'tui')


def rimuovi_campo(testo, campo):
    """Remove the '- **campo:** ...' bullet and its indented continuation lines."""
    lines = testo.split('\n')
    result = []
    skip = False
    for line in lines:
        if re.match(rf'- \*\*{re.escape(campo)}:\*\*', line):
            skip = True
            continue
        if skip:
            # continuation: indented or blank line before the next bullet
            if line.startswith('  ') or (line.strip() == ''):
                continue
            skip = False
        result.append(line)
    # clean up double blank lines
    testo = '\n'.join(result)
    testo = re.sub(r'\n{3,}', '\n\n', testo)
    return testo


def aggiungi_tui(testo, gui_prefix='img/manual/', tui_prefix='img/tui/'):
    """After each GUI image reference, add the matching TUI image if it exists."""
    # find all image references like ![alt](img/manual/xxx.png)
    for match in re.finditer(r'!\[([^\]]*)\]\(' + re.escape(gui_prefix) + r'([^)]+)\)', testo):
        nome = match.group(2)  # e.g. n01-apre.png
        tui_path = os.path.join(TUI_DIR, nome)
        if os.path.exists(tui_path):
            # the alt text for TUI: replace "window"/"finestra" references with terminal
            alt = match.group(1)
            tui_alt = alt
            tui_alt = re.sub(r'\bwindow\b', 'terminal', tui_alt, flags=re.I)
            tui_alt = re.sub(r'\bfinestra\b', 'terminale', tui_alt, flags=re.I)
            tui_ref = f'![{tui_alt}]({tui_prefix}{nome})'
            gui_ref = match.group(0)
            # only add if not already there
            if tui_ref not in testo:
                testo = testo.replace(gui_ref, gui_ref + '\n' + tui_ref)
    return testo


def aggiungi_link(testo, lingua):
    """Add navigation links after the 'how to read' section."""
    if lingua == 'it':
        blocco = (
            '\n> **Due facce sullo stesso motore:** '
            '[il terminale (mrp-tui)](img/tui/n01-apre.png) · '
            '[la finestra (mrp-gui)](img/manual/n01-apre.png) — '
            'le schermate di entrambe sono in ogni carta.\n'
        )
        marcatore = '## 1. Il prodotto in una pagina'
    else:
        blocco = (
            '\n> **Two faces on the same engine:** '
            '[the terminal (mrp-tui)](img/tui/n01-apre.png) · '
            '[the window (mrp-gui)](img/manual/n01-apre.png) — '
            'screenshots of both are in every card.\n'
        )
        marcatore = '## 1. The product in one page'
    if marcatore in testo and blocco not in testo:
        testo = testo.replace('\n' + marcatore, blocco + '\n' + marcatore)
    return testo


def main():
    for nome, lingua, campo in [
        ('docs/MANUAL.it.md', 'it', 'Cosa fa'),
        ('docs/MANUAL.md', 'en', 'What it does'),
    ]:
        percorso = os.path.join(RADICE, nome)
        with open(percorso, 'r', encoding='utf-8') as f:
            testo = f.read()
        n_prima = len(testo)

        testo = rimuovi_campo(testo, campo)
        testo = aggiungi_tui(testo)
        testo = aggiungi_link(testo, lingua)

        with open(percorso, 'w', encoding='utf-8') as f:
            f.write(testo)
        print(f'{nome}: {n_prima} -> {len(testo)} caratteri (via "{campo}", + TUI, + link)')


if __name__ == '__main__':
    main()
