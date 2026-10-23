// Mindcare : fonctions de Windows dont l'appli a besoin (barre de titre sombre, icône dans la barre des tâches,
// affichage dans Mindset, assistant arrêté avec Mindcare). Compilé au démarrage par Mindcare.ps1.
using System;
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
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr CreateJobObject(IntPtr attributs, string nom);
    [DllImport("kernel32.dll")] static extern bool SetInformationJobObject(IntPtr job, int classe, ref LimitesJob infos, int taille);
    [DllImport("kernel32.dll")] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr processus);
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
            if (j == IntPtr.Zero) return false;
            LimitesJob l = new LimitesJob();
            l.LimitFlags = 0x2000;   // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            if (!SetInformationJobObject(j, 9, ref l, Marshal.SizeOf(typeof(LimitesJob)))) return false;   // JobObjectExtendedLimitInformation
            job = j;
        }
        return AssignProcessToJobObject(job, processus);
    }
}
