// Mindcare : fonctions de Windows dont l'appli a besoin (barre de titre sombre, icône dans la barre des tâches,
// affichage dans Mindset). Compilé au démarrage par Mindcare.ps1.
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
}
