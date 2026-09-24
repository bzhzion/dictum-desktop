# Clique a une position DONNEE EN COORDONNEES DE LA FENETRE d’Oyant.
#
# ⛔ Le rectangle est relu a chaque appel. Reutiliser des coordonnees ecran calculees lors d'une
# capture precedente clique a cote des que la fenetre a bouge, et sur ce qu'il y a dessous.
param([int]$X, [int]$Y)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Clic {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint g);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
"@

$processus = Get-Process oyant -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
$poignee = $processus.MainWindowHandle
if (-not [Clic]::IsWindowVisible($poignee)) { throw "fenetre masquee : le clic irait ailleurs" }

$r = New-Object Clic+RECT
[void][Clic]::GetWindowRect($poignee, [ref]$r)
$ecranX = $r.L + $X
$ecranY = $r.T + $Y

# Deuxieme verrou : ce qui se trouve reellement sous le curseur doit appartenir a Oyant.
$point = New-Object Clic+POINT
$point.X = $ecranX; $point.Y = $ecranY
$sous = [Clic]::GetAncestor([Clic]::WindowFromPoint($point), 2)  # GA_ROOT
if ($sous -ne $poignee) { throw "une autre fenetre est au-dessus en $ecranX,$ecranY : clic annule" }

[void][Clic]::SetCursorPos($ecranX, $ecranY)
Start-Sleep -Milliseconds 150
[Clic]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)  # gauche enfonce
Start-Sleep -Milliseconds 60
[Clic]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)  # gauche relache
Write-Output "clic en $ecranX,$ecranY (fenetre $($r.L),$($r.T) + $X,$Y)"
