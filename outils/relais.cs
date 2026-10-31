// Mindcare.exe : un petit relais. Le vrai programme de Mindcare s'appelle Mindset.exe (outils\lanceur.cs), pour que le
// Gestionnaire des tâches range toute la suite sur une seule ligne « Mindset ». Mindcare.exe reste pour tout ce qui
// l'appelle encore (anciennes versions de Mindset, raccourcis, apps.json) : il lance Mindset.exe de son dossier avec les
// mêmes arguments, puis se ferme aussitôt.
// Compilé par construire.ps1.
using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Text;
using System.Windows.Forms;

[assembly: AssemblyTitle("Mindcare")]
[assembly: AssemblyProduct("Mindcare")]
[assembly: AssemblyDescription("Ouvre Mindcare (Mindset.exe, dans le même dossier).")]
[assembly: AssemblyVersion("0.15.1.0")]
[assembly: AssemblyFileVersion("0.15.1.0")]

static class Relais
{
    [STAThread]
    static int Main(string[] args)
    {
        string exe = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Mindset.exe");
        if (!File.Exists(exe))
        {
            Application.EnableVisualStyles();
            MessageBox.Show("Le programme « Mindset.exe » est introuvable à côté de celui-ci.\n\nRéinstalle Mindcare depuis Mindset.",
                "Mindcare", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return 1;
        }
        try
        {
            StringBuilder ligne = new StringBuilder();
            foreach (string a in args) { if (ligne.Length > 0) ligne.Append(' '); ligne.Append(Citer(a)); }
            ProcessStartInfo si = new ProcessStartInfo(exe, ligne.ToString());
            si.UseShellExecute = false;
            si.WorkingDirectory = AppDomain.CurrentDomain.BaseDirectory;
            using (Process.Start(si)) { }
            return 0;
        }
        catch (Exception e)
        {
            Application.EnableVisualStyles();
            MessageBox.Show("Mindcare n'a pas pu s'ouvrir :\n\n" + e.Message, "Mindcare", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }

    // Un argument tel que le programme lancé le relira (règles de Windows : guillemets, et « \ » juste avant un guillemet)
    static string Citer(string a)
    {
        if (a.Length > 0 && a.IndexOfAny(new char[] { ' ', '\t', '"' }) < 0) return a;
        StringBuilder sb = new StringBuilder("\"");
        int barres = 0;
        foreach (char c in a)
        {
            if (c == '\\') { barres++; continue; }
            if (c == '"') { sb.Append('\\', barres * 2 + 1); sb.Append('"'); barres = 0; continue; }
            sb.Append('\\', barres); barres = 0; sb.Append(c);
        }
        sb.Append('\\', barres * 2);
        sb.Append('"');
        return sb.ToString();
    }
}
