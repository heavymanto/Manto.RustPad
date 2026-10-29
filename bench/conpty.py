"""Pilota un'app TUI dentro un ConPTY (senza dipendenze): avvia, invia tasti, misura tempi e memoria.
Uso come modulo: from conpty import Pty"""
import ctypes, threading, time, re
from ctypes import wintypes as wt

k32 = ctypes.WinDLL('kernel32', use_last_error=True)
psapi = ctypes.WinDLL('psapi', use_last_error=True)
H = ctypes.c_void_p

class COORD(ctypes.Structure): _fields_ = [('X', ctypes.c_short), ('Y', ctypes.c_short)]
class STARTUPINFOW(ctypes.Structure):
    _fields_ = [('cb', wt.DWORD), ('lpReserved', wt.LPWSTR), ('lpDesktop', wt.LPWSTR), ('lpTitle', wt.LPWSTR),
                ('dwX', wt.DWORD), ('dwY', wt.DWORD), ('dwXSize', wt.DWORD), ('dwYSize', wt.DWORD),
                ('dwXCountChars', wt.DWORD), ('dwYCountChars', wt.DWORD), ('dwFillAttribute', wt.DWORD),
                ('dwFlags', wt.DWORD), ('wShowWindow', wt.WORD), ('cbReserved2', wt.WORD),
                ('lpReserved2', ctypes.c_void_p), ('hStdInput', H), ('hStdOutput', H), ('hStdError', H)]
class STARTUPINFOEXW(ctypes.Structure): _fields_ = [('StartupInfo', STARTUPINFOW), ('lpAttributeList', ctypes.c_void_p)]
class PROCESS_INFORMATION(ctypes.Structure):
    _fields_ = [('hProcess', H), ('hThread', H), ('dwProcessId', wt.DWORD), ('dwThreadId', wt.DWORD)]
class PMC(ctypes.Structure):
    _fields_ = [('cb', wt.DWORD), ('PageFaultCount', wt.DWORD), ('PeakWorkingSetSize', ctypes.c_size_t),
                ('WorkingSetSize', ctypes.c_size_t), ('QuotaPeakPagedPoolUsage', ctypes.c_size_t),
                ('QuotaPagedPoolUsage', ctypes.c_size_t), ('QuotaPeakNonPagedPoolUsage', ctypes.c_size_t),
                ('QuotaNonPagedPoolUsage', ctypes.c_size_t), ('PagefileUsage', ctypes.c_size_t),
                ('PeakPagefileUsage', ctypes.c_size_t), ('PrivateUsage', ctypes.c_size_t)]

k32.CreatePipe.argtypes = [ctypes.POINTER(H), ctypes.POINTER(H), ctypes.c_void_p, wt.DWORD]
k32.CreatePseudoConsole.argtypes = [COORD, H, H, wt.DWORD, ctypes.POINTER(H)]
k32.ClosePseudoConsole.argtypes = [H]
k32.InitializeProcThreadAttributeList.argtypes = [ctypes.c_void_p, wt.DWORD, wt.DWORD, ctypes.POINTER(ctypes.c_size_t)]
k32.UpdateProcThreadAttribute.argtypes = [ctypes.c_void_p, wt.DWORD, ctypes.c_size_t, H, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p]
k32.CreateProcessW.argtypes = [wt.LPCWSTR, wt.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wt.BOOL, wt.DWORD, ctypes.c_void_p, wt.LPCWSTR, ctypes.POINTER(STARTUPINFOEXW), ctypes.POINTER(PROCESS_INFORMATION)]
k32.ReadFile.argtypes = [H, ctypes.c_void_p, wt.DWORD, ctypes.POINTER(wt.DWORD), ctypes.c_void_p]
k32.WriteFile.argtypes = [H, ctypes.c_void_p, wt.DWORD, ctypes.POINTER(wt.DWORD), ctypes.c_void_p]
k32.SetStdHandle.argtypes = [wt.DWORD, H]
k32.CloseHandle.argtypes = [H]
k32.TerminateProcess.argtypes = [H, wt.UINT]
k32.WaitForSingleObject.argtypes = [H, wt.DWORD]
psapi.GetProcessMemoryInfo.argtypes = [H, ctypes.POINTER(PMC), wt.DWORD]

ANSI = re.compile(rb'\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][A-Z0-9]|\x1b[=>]')

class Pty:
    def __init__(self, cmdline, cols=120, rows=40):
        inR, inW, outR, outW = H(), H(), H(), H()
        k32.CreatePipe(ctypes.byref(inR), ctypes.byref(inW), None, 0)
        k32.CreatePipe(ctypes.byref(outR), ctypes.byref(outW), None, 0)
        self.hpc = H()
        hr = k32.CreatePseudoConsole(COORD(cols, rows), inR, outW, 0, ctypes.byref(self.hpc))
        assert hr == 0, f'CreatePseudoConsole hr={hr:#x}'
        sz = ctypes.c_size_t()
        k32.InitializeProcThreadAttributeList(None, 1, 0, ctypes.byref(sz))
        self._buf = ctypes.create_string_buffer(sz.value)
        k32.InitializeProcThreadAttributeList(self._buf, 1, 0, ctypes.byref(sz))
        assert k32.UpdateProcThreadAttribute(self._buf, 0, 0x20016, self.hpc, ctypes.sizeof(H), None, None), ctypes.get_last_error()
        si = STARTUPINFOEXW(); si.StartupInfo.cb = ctypes.sizeof(si); si.lpAttributeList = ctypes.addressof(self._buf)
        pi = PROCESS_INFORMATION()
        cmd = ctypes.create_unicode_buffer(cmdline)
        for sh in (-10, -11, -12): k32.SetStdHandle(sh, None)   # il figlio non deve ereditare i nostri stdio: deve usare il ConPTY
        self.t0 = time.perf_counter()
        ok = k32.CreateProcessW(None, cmd, None, None, False, 0x80000, None, None, ctypes.byref(si), ctypes.byref(pi))
        assert ok, f'CreateProcess err={ctypes.get_last_error()}'
        self.pi = pi; self.pid = pi.dwProcessId
        k32.CloseHandle(inR); k32.CloseHandle(outW)
        self.inW, self.outR = inW, outR
        self.chunks = []          # (tempo relativo, byte)
        self.lock = threading.Lock()
        self.alive = True
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        buf = ctypes.create_string_buffer(65536); n = wt.DWORD()
        while True:
            if not k32.ReadFile(self.outR, buf, 65536, ctypes.byref(n), None) or n.value == 0: break
            data = buf.raw[:n.value]
            with self.lock: self.chunks.append((time.perf_counter() - self.t0, data))
            if b'\x1b[6n' in data: self.send(b'\x1b[1;1R')      # risposta alla richiesta di posizione del cursore
            if b'\x1b[c' in data or b'\x1b[0c' in data: self.send(b'\x1b[?1;2c')
        self.alive = False

    def send(self, data):
        if isinstance(data, str): data = data.encode()
        n = wt.DWORD(); k32.WriteFile(self.inW, data, len(data), ctypes.byref(n), None)

    def mark(self):
        with self.lock: return len(self.chunks)

    def text_since(self, mark=0):
        with self.lock: raw = b''.join(c for _, c in self.chunks[mark:])
        return ANSI.sub(b'', raw).decode('utf-8', 'replace')

    def wait_for(self, pattern, since=0, timeout=120):
        """secondi (dall'avvio) in cui compare il testo, oppure None"""
        end = time.perf_counter() + timeout
        while time.perf_counter() < end:
            with self.lock:
                for i in range(since, len(self.chunks)):
                    pass
            t = self.text_since(since)
            if re.search(pattern, t):
                with self.lock: return self.chunks[-1][0]
            time.sleep(0.02)
        return None

    def wait_idle(self, since_mark, quiet=0.6, timeout=180):
        """attende che dopo `since_mark` arrivi almeno un output e poi ci sia silenzio per `quiet` s. Ritorna (t_ultimo_output, durata_da_ora)"""
        start = time.perf_counter(); end = start + timeout
        while time.perf_counter() < end:
            with self.lock:
                got = len(self.chunks) > since_mark
                last = self.chunks[-1][0] + self.t0 if self.chunks else 0
            if got and time.perf_counter() - last >= quiet:
                return last - start, time.perf_counter() - start
            time.sleep(0.02)
        return None, None

    def mem(self):
        m = PMC(); m.cb = ctypes.sizeof(m)
        psapi.GetProcessMemoryInfo(self.pi.hProcess, ctypes.byref(m), m.cb)
        mb = lambda x: round(x / 1048576)
        return dict(ws=mb(m.WorkingSetSize), peak_ws=mb(m.PeakWorkingSetSize), private=mb(m.PrivateUsage))

    def running(self):
        return k32.WaitForSingleObject(self.pi.hProcess, 0) == 0x102

    def close(self):
        k32.TerminateProcess(self.pi.hProcess, 1)
        k32.ClosePseudoConsole(self.hpc)
