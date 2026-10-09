param([switch]$Shop,[switch]$NativePurchase,[switch]$Still,[switch]$Terrain,[switch]$Settings,[switch]$Crater,[switch]$Tree,[switch]$Ruin,[switch]$Textures,[string]$Executable='target/iteration/debug/hot-vampires.exe')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $project
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class GameWindow {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref Point p);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
}
'@
function Capture-GameClient($graphics,$handle) {
 $captureDc = $graphics.GetHdc()
 try { if (-not [GameWindow]::PrintWindow($handle,$captureDc,3)) { throw 'Native client capture failed' } }
 finally { $graphics.ReleaseHdc($captureDc) }
}
$arguments = if ($Ruin) { @('--ruin-demo','--frames','1500') } elseif ($Tree) { @('--tree-demo','--frames','1500') } elseif ($Crater) { @('--crater-demo','--frames','1800') } elseif ($Settings) { @('--settings','--frames','1500') } elseif ($Shop) { @('--checkpoint','reports/verified/mobile-11-shop.json','--frames','1500') } elseif ($Terrain) { @('--terrain-walk','--frames','2400') } elseif ($Still) { @('--frames','1500') } else { @('--bot','--frames','1500') }
if ($Textures) { $arguments = @('--ruin-demo','--texture-preview','--frames','1500') }
$process = Start-Process -FilePath "$project/$Executable" -ArgumentList $arguments -WorkingDirectory $project -WindowStyle Hidden -PassThru -RedirectStandardOutput "$project/target/visual-stdout.txt" -RedirectStandardError "$project/target/visual-stderr.txt"
try {
 Start-Sleep -Seconds 2
 if ($Terrain) { Start-Sleep -Seconds 4 }
 $process.Refresh()
 $handle = $process.MainWindowHandle
 if ($handle -eq [IntPtr]::Zero) { throw 'No game window found' }
 [void][GameWindow]::SetForegroundWindow($handle)
 Start-Sleep -Seconds 2
 $rect = New-Object GameWindow+Rect
 [void][GameWindow]::GetClientRect($handle,[ref]$rect)
 $point = New-Object GameWindow+Point
 [void][GameWindow]::ClientToScreen($handle,[ref]$point)
 $bitmap = New-Object System.Drawing.Bitmap ($rect.Right-$rect.Left),($rect.Bottom-$rect.Top)
 $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
 Capture-GameClient $graphics $handle
 New-Item -ItemType Directory -Path reports/visual -Force | Out-Null
 $name = if ($Ruin) { 'ruin' } elseif ($Tree) { 'tree' } elseif ($Crater) { 'crater' } elseif ($Settings) { 'settings' } elseif ($Shop) { 'shop' } elseif ($Terrain) { 'terrain' } elseif ($Still) { 'slow-motion' } else { 'combat' }
 if ($Textures) { $name = 'textures' }
 $bitmap.Save("$project/reports/visual/$name.png")
 $graphics.Dispose()
 $bitmap.Dispose()
 Write-Output "Captured $name client area: $($rect.Right)x$($rect.Bottom)"
 if ($Crater) {
  foreach ($shot in 2..4) {
   Start-Sleep -Seconds 3
   $bitmap = New-Object System.Drawing.Bitmap ($rect.Right-$rect.Left),($rect.Bottom-$rect.Top)
   $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
   Capture-GameClient $graphics $handle
   $bitmap.Save("$project/reports/visual/crater-$shot.png")
   $graphics.Dispose()
   $bitmap.Dispose()
  }
  Write-Output 'Captured crater traversal/orbit sequence (4 frames)'
 }
 if ($Shop -and $NativePurchase) {
  # Exercise the actual egui button using native client mouse messages.
  [void][GameWindow]::SetCursorPos($point.X+672,$point.Y+407)
  Start-Sleep -Milliseconds 150
  [GameWindow]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 150
  [GameWindow]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Seconds 1
  $checkpoint = Get-Content -LiteralPath saves/checkpoint.json -Raw | ConvertFrom-Json
  if (-not ($checkpoint.build | Where-Object { $_.id -eq 'light_frame' })) { throw 'Live UI purchase did not reach the transaction service' }
  if ($checkpoint.currency -ne 85) { throw 'Live UI purchase charged an unexpected amount' }
  Write-Output 'Live UI purchase verified: Light Frame, 105 -> 85 coins, persisted checkpoint'
  $bitmap = New-Object System.Drawing.Bitmap ($rect.Right-$rect.Left),($rect.Bottom-$rect.Top)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  Capture-GameClient $graphics $handle
  $bitmap.Save("$project/reports/visual/shop-purchase.png")
  $graphics.Dispose()
  $bitmap.Dispose()
 }
} finally {
 if (-not $process.HasExited) { [void][GameWindow]::PostMessage($process.MainWindowHandle,0x0010,[IntPtr]::Zero,[IntPtr]::Zero); [void]$process.WaitForExit(5000) }
}
Get-Content -LiteralPath target/visual-stdout.txt
Get-Content -LiteralPath target/visual-stderr.txt
