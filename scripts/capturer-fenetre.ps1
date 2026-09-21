# Capture la fenetre de Dictum, et rien d'autre.
#
# ⛔ On utilise PrintWindow et PAS CopyFromScreen. CopyFromScreen photographie la ZONE DE L'ECRAN
# ou la fenetre est censee se trouver : si elle est masquee, derriere une autre, ou pas au premier
# plan, on capture l'ecran de quelqu'un sans que rien ne le signale. Deja arrive deux fois.
# PrintWindow demande a la fenetre de se dessiner elle-meme dans notre image : ce qui n'est pas
# la fenetre ne peut pas s'y retrouver.
param([string]$Sortie, [string]$NomProcessus = 'dictum')

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Fen {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@

# ⚠️ Nom different du parametre : PowerShell ignore la casse, donc $Processus et
# $processus seraient la MEME variable, et le parametre serait ecrase.
$trouve = Get-Process $NomProcessus -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $trouve) { throw "$NomProcessus n'a pas de fenetre visible" }
$poignee = $trouve.MainWindowHandle

if ([Fen]::IsIconic($poignee)) { [void][Fen]::ShowWindow($poignee, 9) }  # SW_RESTORE
if (-not [Fen]::IsWindowVisible($poignee)) {
  throw "la fenetre de $NomProcessus est masquee : rien a capturer"
}

$r = New-Object Fen+RECT
[void][Fen]::GetWindowRect($poignee, [ref]$r)
$largeur = $r.R - $r.L
$hauteur = $r.B - $r.T
if ($largeur -le 0 -or $hauteur -le 0) { throw "fenetre de taille nulle" }

$image = New-Object System.Drawing.Bitmap $largeur, $hauteur
$graphique = [System.Drawing.Graphics]::FromImage($image)
$dc = $graphique.GetHdc()
# 0x2 = PW_RENDERFULLCONTENT, indispensable pour une fenetre composee (WebView2).
$ok = [Fen]::PrintWindow($poignee, $dc, 2)
$graphique.ReleaseHdc($dc)
if (-not $ok) { $graphique.Dispose(); $image.Dispose(); throw "PrintWindow a refuse" }

$image.Save($Sortie, [System.Drawing.Imaging.ImageFormat]::Png)
$graphique.Dispose(); $image.Dispose()

Write-Output "$Sortie ($largeur x $hauteur)"
