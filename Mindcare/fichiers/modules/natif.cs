// Mindcare : fonctions de Windows dont l'appli a besoin (barre de titre sombre, icône dans la barre des tâches,
// affichage dans Mindset, assistant arrêté avec Mindcare). Compilé au démarrage par Mindcare.ps1.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

public static class MindcareNatif
{
    [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int value, int size);
    [DllImport("shell32.dll", PreserveSig = false)] static extern void SetCurrentProcessExplicitAppUserModelID([MarshalAs(UnmanagedType.LPWStr)] string appId);
    [DllImport("user32.dll")] static extern int GetWindowLong(IntPtr hwnd, int index);
    [DllImport("user32.dll")] static extern IntPtr GetParent(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")] static extern bool ScreenToClient(IntPtr hwnd, ref POINT p);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hwnd, IntPtr apres, int x, int y, int l, int h, uint flags);
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }

    // Barre de titre sombre, et sur Windows 11 de la couleur du fond (couleur en « 0xBBGGRR »)
    public static void SetDarkTitleBar(IntPtr hwnd, int captionBgr)
    {
        int on = 1;
        if (DwmSetWindowAttribute(hwnd, 20, ref on, 4) != 0) DwmSetWindowAttribute(hwnd, 19, ref on, 4);
        DwmSetWindowAttribute(hwnd, 35, ref captionBgr, 4);
    }

    // Identité propre dans la barre des tâches (son icône, pas celle de PowerShell)
    public static void SetAppId(string appId) { SetCurrentProcessExplicitAppUserModelID(appId); }

    // Identité de la fenêtre dans la barre des tâches, et ce que Windows relance quand on l'épingle : Mindset.exe de son
    // dossier (son hôte), avec le nom « Mindcare » et son icône, pas PowerShell ni « Mindset » (le nom de l'hôte).
    // (Le nom du bouton lui-même vient du raccourci du menu Démarrer de même identité, écrit par Mindset.)
    // Même principe que Mindset, Nevermind et Evermind.
    [StructLayout(LayoutKind.Sequential, Pack = 4)] struct PROPERTYKEY { public Guid fmtid; public uint pid; }
    [StructLayout(LayoutKind.Explicit)] struct PROPVARIANT
    {
        [FieldOffset(0)] public ushort vt;
        [FieldOffset(8)] public IntPtr p;
        [FieldOffset(8)] public long pad1;
        [FieldOffset(16)] public long pad2;
    }
    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore
    {
        void GetCount(out uint count);
        void GetAt(uint index, out PROPERTYKEY key);
        void GetValue(ref PROPERTYKEY key, out PROPVARIANT value);
        void SetValue(ref PROPERTYKEY key, ref PROPVARIANT value);
        void Commit();
    }
    [DllImport("shell32.dll", PreserveSig = false)] static extern void SHGetPropertyStoreForWindow(IntPtr hwnd, ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out IPropertyStore store);

    static void SetString(IPropertyStore store, uint pid, string value)
    {
        PROPERTYKEY key = new PROPERTYKEY { fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), pid = pid };
        PROPVARIANT v = new PROPVARIANT { vt = 31, p = Marshal.StringToCoTaskMemUni(value) };   // 31 = VT_LPWSTR
        try { store.SetValue(ref key, ref v); }
        finally { Marshal.FreeCoTaskMem(v.p); }
    }

    public static void SetWindowIdentity(IntPtr hwnd, string appId, string relaunchCommand, string iconPath, string name)
    {
        Guid iid = typeof(IPropertyStore).GUID;
        IPropertyStore store;
        SHGetPropertyStoreForWindow(hwnd, ref iid, out store);
        try
        {
            SetString(store, 2, relaunchCommand);   // System.AppUserModel.RelaunchCommand
            SetString(store, 3, iconPath);          // System.AppUserModel.RelaunchIconResource
            SetString(store, 4, name);              // System.AppUserModel.RelaunchDisplayNameResource
            SetString(store, 5, appId);             // System.AppUserModel.ID, en dernier
            store.Commit();
        }
        finally { Marshal.ReleaseComObject(store); }
    }

    // Accrochée dans Mindset : style WS_CHILD, ou un parent
    public static bool EstEnfant(IntPtr hwnd)
    {
        return hwnd != IntPtr.Zero && ((GetWindowLong(hwnd, -16) & 0x40000000) != 0 || GetParent(hwnd) != IntPtr.Zero);
    }

    // Place de la fenêtre dans son parent (x, y, largeur, hauteur), pour la lui rendre après l'avoir montrée
    public static int[] PlaceDansParent(IntPtr hwnd)
    {
        RECT r; if (!GetWindowRect(hwnd, out r)) return null;
        POINT p = new POINT { X = r.Left, Y = r.Top };
        IntPtr parent = GetParent(hwnd);
        if (parent != IntPtr.Zero) ScreenToClient(parent, ref p);
        return new int[] { p.X, p.Y, r.Right - r.Left, r.Bottom - r.Top };
    }
    public static void Placer(IntPtr hwnd, int[] place)
    {
        if (place != null) SetWindowPos(hwnd, IntPtr.Zero, place[0], place[1], place[2], place[3], 0x0014);   // SWP_NOZORDER | SWP_NOACTIVATE
    }

    // L'assistant (llama-server) s'arrête avec Mindcare, même quand Mindcare est arrêtée de force (Mindset la ferme ainsi
    // avant une mise à jour ou une désinstallation) : il est mis dans un « job » de Windows qui arrête tout ce qu'il
    // contient quand le dernier accès au job se ferme, c'est-à-dire quand le processus de Mindcare disparaît.
    // (Erreur : la cause d'un échec, avec le code d'erreur de Windows, pour le journal ; comme EMJob d'Evermind)
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr CreateJobObject(IntPtr attributs, string nom);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetInformationJobObject(IntPtr job, int classe, ref LimitesJob infos, int taille);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr processus);
    public static string Erreur = "";
    [StructLayout(LayoutKind.Sequential)] struct LimitesJob
    {
        public long PerProcessUserTimeLimit, PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass, SchedulingClass;
        public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount, ReadTransferCount, WriteTransferCount, OtherTransferCount;
        public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed;
    }
    static IntPtr job = IntPtr.Zero;   // (jamais fermé à la main : Windows le ferme quand Mindcare s'arrête)
    public static bool ArreterAvecMindcare(IntPtr processus)
    {
        if (job == IntPtr.Zero)
        {
            IntPtr j = CreateJobObject(IntPtr.Zero, null);
            if (j == IntPtr.Zero) { Erreur = "création du job (code " + Marshal.GetLastWin32Error() + ")"; return false; }
            LimitesJob l = new LimitesJob();
            l.LimitFlags = 0x2000;   // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            if (!SetInformationJobObject(j, 9, ref l, Marshal.SizeOf(typeof(LimitesJob))))   // JobObjectExtendedLimitInformation
            { Erreur = "réglage du job (code " + Marshal.GetLastWin32Error() + ")"; return false; }
            job = j;
        }
        if (!AssignProcessToJobObject(job, processus)) { Erreur = "ajout au job (code " + Marshal.GetLastWin32Error() + ")"; return false; }
        return true;
    }
}

// Les dernières lignes d'erreur d'un programme lancé (l'assistant) : pour noter au journal pourquoi il s'est arrêté. En C#
// parce que ces lignes arrivent sur un autre fil, où un bloc de script PowerShell ne peut pas tourner.
public class MindcareSorties
{
    readonly Queue<string> lignes = new Queue<string>();
    readonly int max;
    public MindcareSorties(Process p, int max)
    {
        this.max = max;
        p.ErrorDataReceived += Recevoir;
        p.OutputDataReceived += Recevoir;
    }
    void Recevoir(object s, DataReceivedEventArgs e)
    {
        if (e.Data == null) return;
        lock (lignes) { lignes.Enqueue(e.Data); while (lignes.Count > max) lignes.Dequeue(); }
    }
    public string[] Lignes() { lock (lignes) { return lignes.ToArray(); } }
}

// ---------------------------------------------------------------------------
// La dictée (dictee.ps1) : le micro choisi dans Mindset (par son nom), sinon celui de Windows. Enregistre en 16 kHz mono,
// ce que Whisper lit, et suit le niveau du son pour savoir quand on a fini de parler. Le même code qu'Evermind (EMMicro).
// ---------------------------------------------------------------------------
public class MindcareMicro
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct CAPS { public ushort mid, pid; public uint ver; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string name; public uint fmts; public ushort ch, res; }
    [StructLayout(LayoutKind.Sequential)]
    struct WAVEFORMATEX { public ushort wFormatTag, nChannels; public uint nSamplesPerSec, nAvgBytesPerSec; public ushort nBlockAlign, wBitsPerSample, cbSize; }
    [StructLayout(LayoutKind.Sequential)]
    struct WAVEHDR { public IntPtr lpData; public uint dwBufferLength, dwBytesRecorded; public IntPtr dwUser; public uint dwFlags, dwLoops; public IntPtr lpNext, reserved; }

    [DllImport("winmm.dll")] static extern uint waveInGetNumDevs();
    [DllImport("winmm.dll", CharSet = CharSet.Unicode)] static extern int waveInGetDevCapsW(IntPtr id, ref CAPS c, int sz);
    [DllImport("winmm.dll")] static extern int waveInOpen(out IntPtr h, uint dev, ref WAVEFORMATEX f, IntPtr cb, IntPtr inst, uint flags);
    [DllImport("winmm.dll")] static extern int waveInPrepareHeader(IntPtr h, IntPtr hdr, uint size);
    [DllImport("winmm.dll")] static extern int waveInUnprepareHeader(IntPtr h, IntPtr hdr, uint size);
    [DllImport("winmm.dll")] static extern int waveInAddBuffer(IntPtr h, IntPtr hdr, uint size);
    [DllImport("winmm.dll")] static extern int waveInStart(IntPtr h);
    [DllImport("winmm.dll")] static extern int waveInReset(IntPtr h);
    [DllImport("winmm.dll")] static extern int waveInClose(IntPtr h);

    const uint WAVE_MAPPER = 0xFFFFFFFF, CALLBACK_EVENT = 0x50000, WHDR_DONE = 1;
    const int Rate = 16000, BufBytes = 3200;   // tampons de 100 ms

    IntPtr h = IntPtr.Zero;
    readonly IntPtr[] hdrs = new IntPtr[4];
    readonly System.Threading.AutoResetEvent ev = new System.Threading.AutoResetEvent(false);
    readonly System.IO.MemoryStream data = new System.IO.MemoryStream();
    System.Threading.Thread th;
    volatile bool run;
    double floor = 0; int floorCount = 0;

    public volatile float Level;              // niveau du son, de 0 à 1 (pour l'animation)
    public volatile int Millis;               // durée enregistrée
    public volatile bool HeardVoice;          // on a entendu parler
    public volatile int MillisSinceVoice;     // depuis combien de temps on s'est tu

    // Noms des micros (Windows les coupe à 31 lettres), dans l'ordre de leurs numéros
    public static string[] Noms()
    {
        uint n = waveInGetNumDevs();
        string[] r = new string[n];
        for (uint d = 0; d < n; d++) { CAPS c = new CAPS(); waveInGetDevCapsW((IntPtr)d, ref c, Marshal.SizeOf(c)); r[d] = c.name ?? ""; }
        return r;
    }
    public static bool HasDevice() { return waveInGetNumDevs() > 0; }
    static int HeaderSize { get { return Marshal.SizeOf(typeof(WAVEHDR)); } }

    // 0 si l'enregistrement a commencé, sinon le code d'erreur de Windows (micro absent ou refusé).
    // Micro : son numéro (Noms), ou 0xFFFFFFFF pour celui de Windows.
    public int Start(uint micro)
    {
        WAVEFORMATEX f = new WAVEFORMATEX { wFormatTag = 1, nChannels = 1, nSamplesPerSec = Rate, wBitsPerSample = 16, nBlockAlign = 2, nAvgBytesPerSec = Rate * 2, cbSize = 0 };
        int r = waveInOpen(out h, micro, ref f, ev.SafeWaitHandle.DangerousGetHandle(), IntPtr.Zero, CALLBACK_EVENT);
        if (r != 0) { h = IntPtr.Zero; return r; }
        for (int i = 0; i < hdrs.Length; i++)
        {
            WAVEHDR hd = new WAVEHDR { lpData = Marshal.AllocHGlobal(BufBytes), dwBufferLength = BufBytes };
            hdrs[i] = Marshal.AllocHGlobal(HeaderSize);
            Marshal.StructureToPtr(hd, hdrs[i], false);
            waveInPrepareHeader(h, hdrs[i], (uint)HeaderSize);
            waveInAddBuffer(h, hdrs[i], (uint)HeaderSize);
        }
        run = true;
        th = new System.Threading.Thread(Loop) { IsBackground = true };
        th.Start();
        return waveInStart(h);
    }

    void Loop()
    {
        while (run)
        {
            ev.WaitOne(200);
            for (int i = 0; i < hdrs.Length && run; i++)
            {
                WAVEHDR hd = (WAVEHDR)Marshal.PtrToStructure(hdrs[i], typeof(WAVEHDR));
                if ((hd.dwFlags & WHDR_DONE) == 0) continue;
                int n = (int)hd.dwBytesRecorded;
                if (n > 0)
                {
                    byte[] b = new byte[n];
                    Marshal.Copy(hd.lpData, b, 0, n);
                    lock (data) data.Write(b, 0, n);
                    Analyse(b, n);
                }
                waveInUnprepareHeader(h, hdrs[i], (uint)HeaderSize);
                hd.dwFlags = 0; hd.dwBytesRecorded = 0;
                Marshal.StructureToPtr(hd, hdrs[i], false);
                waveInPrepareHeader(h, hdrs[i], (uint)HeaderSize);
                waveInAddBuffer(h, hdrs[i], (uint)HeaderSize);
            }
        }
    }

    // Niveau du son ; le bruit de fond est mesuré sur les 300 premières millisecondes
    void Analyse(byte[] b, int n)
    {
        double sum = 0; int count = n / 2;
        for (int i = 0; i + 1 < n; i += 2) { short s = (short)(b[i] | (b[i + 1] << 8)); sum += (double)s * s; }
        double rms = count > 0 ? Math.Sqrt(sum / count) / 32768.0 : 0;
        int ms = count * 1000 / Rate;
        Millis += ms;
        Level = (float)Math.Min(1.0, rms * 8);
        if (floorCount < 3) { floor = (floor * floorCount + rms) / (floorCount + 1); floorCount++; return; }
        double seuil = Math.Max(0.012, floor * 3);
        if (rms > seuil) { HeardVoice = true; MillisSinceVoice = 0; }
        else MillisSinceVoice += ms;
    }

    public void Stop()
    {
        if (h == IntPtr.Zero) return;
        run = false;
        ev.Set();
        if (th != null) th.Join(1000);
        waveInReset(h);
        for (int i = 0; i < hdrs.Length; i++)
        {
            if (hdrs[i] == IntPtr.Zero) continue;
            WAVEHDR hd = (WAVEHDR)Marshal.PtrToStructure(hdrs[i], typeof(WAVEHDR));
            waveInUnprepareHeader(h, hdrs[i], (uint)HeaderSize);
            Marshal.FreeHGlobal(hd.lpData);
            Marshal.FreeHGlobal(hdrs[i]);
            hdrs[i] = IntPtr.Zero;
        }
        waveInClose(h);
        h = IntPtr.Zero;
    }

    // Enregistre ce qui a été entendu dans un fichier .wav (effacé dès que Whisper l'a lu)
    public void SaveWav(string path)
    {
        byte[] pcm; lock (data) pcm = data.ToArray();
        using (System.IO.BinaryWriter w = new System.IO.BinaryWriter(System.IO.File.Create(path)))
        {
            w.Write(new[] { 'R', 'I', 'F', 'F' }); w.Write(36 + pcm.Length);
            w.Write(new[] { 'W', 'A', 'V', 'E', 'f', 'm', 't', ' ' }); w.Write(16);
            w.Write((short)1); w.Write((short)1); w.Write(Rate); w.Write(Rate * 2); w.Write((short)2); w.Write((short)16);
            w.Write(new[] { 'd', 'a', 't', 'a' }); w.Write(pcm.Length); w.Write(pcm);
        }
    }
}
