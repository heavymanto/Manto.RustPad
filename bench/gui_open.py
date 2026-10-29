"""Misura apertura e memoria di un editor/viewer GUI Windows su un file grande.

Uso:  python -I -X utf8 gui_open.py <nome> <file> [--end] [--out risultati.jsonl]
      <nome> = npp | ltf | ue   (percorsi in APPS)

Per ogni esecuzione: lancia il programma con il file, osserva il processo ogni 100 ms
(working set, memoria privata, byte letti da disco, CPU, finestre visibili e testo delle finestre di dialogo)
e termina SOLO il processo avviato. Con --end invia Ctrl+End e misura quanto impiega a calmarsi.
Non scrive nulla sul file di prova.
"""
import ctypes, json, os, subprocess, sys, time
from ctypes import wintypes as wt

# percorsi di installazione: sovrascrivibili con MRP_BENCH_NPP / MRP_BENCH_LTF / MRP_BENCH_UE
APPS = {
    'npp': (os.environ.get('MRP_BENCH_NPP', r'C:\Program Files\Notepad++\notepad++.exe'), ['-multiInst', '-nosession', '-noPlugin']),
    'ltf': (os.environ.get('MRP_BENCH_LTF', r'C:\Program Files\LTFViewr\LTFViewr5u.exe'), []),
    'ue':  (os.environ.get('MRP_BENCH_UE', r'C:\Program Files (x86)\UltraEdit\uedit32.exe'), []),
}
u32 = ctypes.WinDLL('user32', use_last_error=True); k32 = ctypes.WinDLL('kernel32', use_last_error=True)
psapi = ctypes.WinDLL('psapi', use_last_error=True)
H = ctypes.c_void_p
k32.OpenProcess.restype = H; k32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
k32.CloseHandle.argtypes = [H]

class PMC(ctypes.Structure):
    _fields_ = [('cb', wt.DWORD), ('PageFaultCount', wt.DWORD), ('PeakWorkingSetSize', ctypes.c_size_t),
                ('WorkingSetSize', ctypes.c_size_t), ('QuotaPeakPagedPoolUsage', ctypes.c_size_t),
                ('QuotaPagedPoolUsage', ctypes.c_size_t), ('QuotaPeakNonPagedPoolUsage', ctypes.c_size_t),
                ('QuotaNonPagedPoolUsage', ctypes.c_size_t), ('PagefileUsage', ctypes.c_size_t),
                ('PeakPagefileUsage', ctypes.c_size_t), ('PrivateUsage', ctypes.c_size_t)]
class IOC(ctypes.Structure):
    _fields_ = [(n, ctypes.c_ulonglong) for n in ('ReadOps', 'WriteOps', 'OtherOps', 'ReadBytes', 'WriteBytes', 'OtherBytes')]
class FT(ctypes.Structure): _fields_ = [('lo', wt.DWORD), ('hi', wt.DWORD)]
psapi.GetProcessMemoryInfo.argtypes = [H, ctypes.POINTER(PMC), wt.DWORD]
k32.GetProcessIoCounters.argtypes = [H, ctypes.POINTER(IOC)]
k32.GetProcessTimes.argtypes = [H, ctypes.POINTER(FT), ctypes.POINTER(FT), ctypes.POINTER(FT), ctypes.POINTER(FT)]

ENUMPROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

def windows_of(pid):
    """(titolo, classe, [testi figli]) delle finestre di primo livello visibili del processo"""
    out = []
    def child_texts(hwnd):
        texts = []
        def cb(h, _):
            n = u32.GetWindowTextLengthW(h)
            if n:
                b = ctypes.create_unicode_buffer(n + 1); u32.GetWindowTextW(h, b, n + 1); texts.append(b.value)
            return True
        u32.EnumChildWindows(hwnd, ENUMPROC(cb), 0)
        return texts
    def cb(h, _):
        p = wt.DWORD(); u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u32.IsWindowVisible(h):
            n = u32.GetWindowTextLengthW(h); b = ctypes.create_unicode_buffer(n + 1); u32.GetWindowTextW(h, b, n + 1)
            c = ctypes.create_unicode_buffer(64); u32.GetClassNameW(h, c, 64)
            out.append((b.value, c.value, child_texts(h) if c.value == '#32770' else [], h))
        return True
    u32.EnumWindows(ENUMPROC(cb), 0)
    return out


AUTO = {  # titolo del dialogo -> testo del pulsante da premere (risposta "normale" di un utente)
    'Apertura file di grandi dimensioni': '&Sì',      # Notepad++: prosegui
    'File is probably not DOS format': '&No',         # UltraEdit: non convertire
    'UltraEdit File Associations': 'Cancel',          # UltraEdit: primo avvio
}
ENUMCHILD = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

def click_button(dlg, label):
    """preme (BM_CLICK) il pulsante figlio con quel testo; True se trovato"""
    found = []
    def cb(h, _):
        n = u32.GetWindowTextLengthW(h)
        if n:
            b = ctypes.create_unicode_buffer(n + 1); u32.GetWindowTextW(h, b, n + 1)
            if b.value == label: found.append(h)
        return True
    u32.EnumChildWindows(dlg, ENUMCHILD(cb), 0)
    if found: u32.PostMessageW(found[0], 0x00F5, 0, 0); return True
    return False

def open_dialogs(pid):
    return [(t, h) for (t, c, _, h) in windows_of(pid) if c == '#32770']

def sample(hproc):
    m = PMC(); m.cb = ctypes.sizeof(m); psapi.GetProcessMemoryInfo(hproc, ctypes.byref(m), m.cb)
    io = IOC(); k32.GetProcessIoCounters(hproc, ctypes.byref(io))
    c, e, k, u = FT(), FT(), FT(), FT(); k32.GetProcessTimes(hproc, ctypes.byref(c), ctypes.byref(e), ctypes.byref(k), ctypes.byref(u))
    cpu = ((k.hi << 32 | k.lo) + (u.hi << 32 | u.lo)) / 1e7          # secondi di CPU
    mb = lambda x: round(x / 1048576)
    return dict(ws=mb(m.WorkingSetSize), peak_ws=mb(m.PeakWorkingSetSize), private=mb(m.PrivateUsage),
                read_mb=mb(io.ReadBytes), write_mb=mb(io.WriteBytes), cpu_s=round(cpu, 2))

def settle(hproc, pid, quiet=1.0, timeout=180, min_wait=0.3):
    answered_local = set()
    """attende che CPU e lettura disco siano ferme per `quiet` secondi; ritorna (secondi, campione)"""
    t0 = time.perf_counter(); last = None; still_since = None
    while time.perf_counter() - t0 < timeout:
        for title, h in open_dialogs(pid):
            if title in AUTO and (title, h) not in answered_local and click_button(h, AUTO[title]): answered_local.add((title, h))
        s = sample(hproc); key = (s['cpu_s'], s['read_mb'], s['write_mb'])
        if last is not None and key == last:
            still_since = still_since or time.perf_counter()
            if time.perf_counter() - still_since >= quiet and time.perf_counter() - t0 >= min_wait:
                return round(still_since - t0, 2), s
        else: still_since = None
        last = key; time.sleep(0.1)
    return None, sample(hproc)


def statusbar_text(pid, main_hwnd, hproc):
    """testo di ogni parte della status bar nativa (msctls_statusbar32) del processo; lista vuota se assente"""
    sb = []
    def cb(h, _):
        c = ctypes.create_unicode_buffer(64); u32.GetClassNameW(h, c, 64)
        if c.value == 'msctls_statusbar32': sb.append(h)
        return True
    u32.EnumChildWindows(main_hwnd, ENUMCHILD(cb), 0)
    if not sb: return []
    k32.VirtualAllocEx.restype = ctypes.c_void_p
    k32.VirtualAllocEx.argtypes = [H, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, wt.DWORD]
    k32.ReadProcessMemory.argtypes = [H, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
    u32.SendMessageW.restype = ctypes.c_ssize_t; u32.SendMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_void_p]
    remote = k32.VirtualAllocEx(hproc, None, 4096, 0x3000, 4)
    out = []
    for i in range(u32.SendMessageW(sb[0], 0x0406, 0, None)):               # SB_GETPARTS
        n = u32.SendMessageW(sb[0], 0x040C, i, None) & 0xFFFF                # SB_GETTEXTLENGTHW
        if n == 0: out.append(''); continue
        u32.SendMessageW(sb[0], 0x040D, i, remote)                           # SB_GETTEXTW nel buffer remoto
        buf = ctypes.create_string_buffer(2 * n + 2); got = ctypes.c_size_t()
        k32.ReadProcessMemory(hproc, remote, buf, 2 * n + 2, ctypes.byref(got))
        out.append(buf.raw.decode('utf-16le', 'ignore').split('\0')[0])
    return out

def key_ctrl_end(hwnd):
    u32.keybd_event(0x12, 0, 0, 0); u32.keybd_event(0x12, 0, 2, 0)      # un tocco di Alt sblocca SetForegroundWindow
    u32.ShowWindow(hwnd, 9); u32.SetForegroundWindow(hwnd); time.sleep(0.6)
    if u32.GetForegroundWindow() != hwnd: return False
    VK_CTRL, VK_END, KEYUP = 0x11, 0x23, 2
    u32.keybd_event(VK_CTRL, 0, 0, 0); u32.keybd_event(VK_END, 0, 1, 0)       # 1 = EXTENDEDKEY
    u32.keybd_event(VK_END, 0, 1 | KEYUP, 0); u32.keybd_event(VK_CTRL, 0, KEYUP, 0)
    return True

def main():
    name, path = sys.argv[1], sys.argv[2]
    out = sys.argv[sys.argv.index('--out') + 1] if '--out' in sys.argv else None
    exe, extra = APPS[name]; base = os.path.basename(path).split('.')[0]
    res = dict(app=name, file=os.path.basename(path), file_mb=round(os.path.getsize(path) / 1e6))
    t0 = time.perf_counter()
    proc = subprocess.Popen([exe] + extra + [path])
    pid = proc.pid; hproc = k32.OpenProcess(0x1F0FFF, False, pid)
    t_title = None; dialogs = {}; titles = set(); peak = 0; answered = {}
    deadline = t0 + 120
    while time.perf_counter() < deadline and proc.poll() is None:
        for title, cls, texts, h in windows_of(pid):
            titles.add(title)
            if cls == '#32770':
                dialogs[title] = texts
                if title in AUTO and title not in answered and click_button(h, AUTO[title]): answered[title] = round(time.perf_counter() - t0, 2)
            if t_title is None and base.lower() in title.lower(): t_title = round(time.perf_counter() - t0, 2)
        s = sample(hproc); peak = max(peak, s['private'])
        if t_title is not None and time.perf_counter() - t0 > t_title + 2: break
        time.sleep(0.1)
    res['alive'] = proc.poll() is None
    res['t_title_s'] = t_title
    t_set, s = settle(hproc, pid, quiet=1.5, timeout=120)
    res['t_settled_s'] = None if t_set is None else round(t_set + (time.perf_counter() - t0 - t_set) * 0, 2)
    res['t_since_launch_s'] = round(time.perf_counter() - t0, 2)
    res['after_open'] = s
    res['titles'] = sorted(t for t in titles if t)[:6]
    res['dialogs_answered_at_s'] = answered
    res['dialogs'] = {k: v for k, v in dialogs.items()}
    if res['alive']:
        try:
            mains = [w for w in windows_of(pid) if w[0] not in AUTO]
            res['statusbar'] = statusbar_text(pid, mains[0][3], hproc) if mains else []
        except Exception as e: res['statusbar'] = f'errore: {e}'
    if '--end' in sys.argv and res['alive']:
        allw = windows_of(pid)
        wins = [w for w in allw if w[1] != '#32770'] or [w for w in allw if w[0] not in AUTO][:1]   # LTFViewer e' esso stesso una dialog
        res['dialogs_open_at_end'] = [t for t, _ in open_dialogs(pid) if t in AUTO]
        wins.sort(key=lambda w: base.lower() not in w[0].lower())          # prima la finestra che porta il nome del file
        if wins and key_ctrl_end(wins[0][3]):
            t1 = time.perf_counter(); r0 = sample(hproc)['read_mb']; w0 = sample(hproc)['write_mb']
            dt, s2 = settle(hproc, pid, quiet=0.8, timeout=120, min_wait=0.5)
            res['ctrl_end'] = dict(settle_s=dt, extra_read_mb=s2['read_mb'] - r0, extra_write_mb=s2['write_mb'] - w0, mem=s2)
        else: res['ctrl_end'] = 'finestra non in primo piano: tasto non inviato'
    subprocess.run(['taskkill', '/PID', str(pid), '/T', '/F'], capture_output=True)
    k32.CloseHandle(hproc)
    line = json.dumps(res, ensure_ascii=False)
    print(line)
    if out:
        with open(out, 'a', encoding='utf-8') as f: f.write(line + '\n')

if __name__ == '__main__':
    main()
