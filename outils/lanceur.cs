// Mindcare.exe : le programme Mindcare. Il exécute lui-même le code de l'appli (fichiers\Mindcare.ps1) au lieu de le
// confier à powershell.exe : pour Windows, c'est une vraie application, avec son nom et son icône.
// Compilé par construire.ps1. « Mindcare.exe -Demarrage » : lancée cachée (par Mindset, ou avec Windows).
using System;
using System.IO;
using System.Management.Automation;
using System.Management.Automation.Runspaces;
using System.Reflection;
using System.Text;
using System.Threading;
using System.Windows.Forms;

[assembly: AssemblyTitle("Mindcare")]
[assembly: AssemblyProduct("Mindcare")]
[assembly: AssemblyDescription("Trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.")]
[assembly: AssemblyVersion("0.3.0.0")]
[assembly: AssemblyFileVersion("0.3.0.0")]

static class Lanceur
{
    [STAThread]
    static int Main(string[] args)
    {
        Application.EnableVisualStyles();
        string dossier = AppDomain.CurrentDomain.BaseDirectory;
        string script = Path.Combine(Path.Combine(dossier, "fichiers"), "Mindcare.ps1");
        if (!File.Exists(script))
        {
            MessageBox.Show("Le dossier « fichiers » est introuvable à côté de ce programme.\n\nRéinstalle Mindcare depuis Mindset.",
                "Mindcare", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return 1;
        }
        bool demarrage = Array.Exists(args, a => string.Equals(a, "-Demarrage", StringComparison.OrdinalIgnoreCase));
        try
        {
            // Le code tourne sur ce fil (STA, nécessaire aux fenêtres), sans les restrictions de scripts de Windows
            InitialSessionState iss = InitialSessionState.CreateDefault();
            iss.ExecutionPolicy = Microsoft.PowerShell.ExecutionPolicy.Bypass;
            using (Runspace rs = RunspaceFactory.CreateRunspace(iss))
            {
                rs.ApartmentState = ApartmentState.STA;
                rs.ThreadOptions = PSThreadOptions.UseCurrentThread;
                rs.Open();
                using (PowerShell ps = PowerShell.Create())
                {
                    ps.Runspace = rs;
                    ps.AddCommand(script);
                    if (demarrage) ps.AddParameter("Demarrage");
                    ps.Invoke();
                    if (ps.Streams.Error.Count > 0)
                    {
                        StringBuilder sb = new StringBuilder();
                        foreach (ErrorRecord e in ps.Streams.Error) sb.AppendLine(e.ToString() + " (" + e.InvocationInfo.PositionMessage + ")");
                        Journal("Erreurs : " + sb);
                    }
                }
            }
        }
        catch (Exception e)
        {
            Journal("Arrêt inattendu : " + e);
            MessageBox.Show("Mindcare s'est arrêtée à cause d'une erreur :\n\n" + e.Message, "Mindcare", MessageBoxButtons.OK, MessageBoxIcon.Error);
            Environment.Exit(1);
        }
        // L'appli a fini : le processus se termine pour de bon. Sans cela, un fil de fond (minuterie, travail en arrière-plan)
        // peut le garder en vie sans fenêtre, avec son verrou : Mindset ne pourrait plus la rouvrir (bug de Nevermind, 2026).
        Environment.Exit(0);
        return 0;
    }

    // Même journal que le code de l'appli : il donne son chemin dans MINDCARE_JOURNAL (modules\donnees.ps1).
    // S'il s'est arrêté avant : <X>:\Mindset\Donnees\Mindcare (Mindset sur un autre disque), sinon %LOCALAPPDATA%\Mindset\Mindcare.
    static void Journal(string texte)
    {
        try
        {
            string fichier = Environment.GetEnvironmentVariable("MINDCARE_JOURNAL");
            if (string.IsNullOrEmpty(fichier) || !fichier.EndsWith("journal.txt", StringComparison.OrdinalIgnoreCase))
            {
                string dir = Path.Combine(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Mindset"), "Mindcare");
                DirectoryInfo racine = new DirectoryInfo(AppDomain.CurrentDomain.BaseDirectory.TrimEnd('\\'));
                if (racine.Parent != null && racine.Parent.Parent != null && string.Equals(racine.Parent.Name, "Programmes", StringComparison.OrdinalIgnoreCase)
                    && string.Equals(racine.Parent.Parent.Name, "Mindset", StringComparison.OrdinalIgnoreCase))
                    dir = Path.Combine(Path.Combine(racine.Parent.Parent.FullName, "Donnees"), "Mindcare");
                fichier = Path.Combine(dir, "journal.txt");
            }
            Directory.CreateDirectory(Path.GetDirectoryName(fichier));
            File.AppendAllText(fichier, DateTime.Now.ToString("s") + " " + texte + "\r\n", new UTF8Encoding(false));
        }
        catch { }
    }
}
