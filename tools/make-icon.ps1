# Draws the app icon (a knob with an accent arc) and writes
#   <OutDir>\icon.png  (256 px)
#   <OutDir>\icon.ico  (16 - 256 px, PNG-compressed entries)
#
# Usage:  powershell -ExecutionPolicy Bypass -File tools\make-icon.ps1 -OutDir crates\knob-settings\icons

param([Parameter(Mandatory = $true)][string]$OutDir)

Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function New-KnobBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $s = $size / 256.0

    # Accent arc: 270 degrees, open at the bottom, three quarters "filled".
    $arcWidth = [Math]::Max(1.5, 20 * $s)
    $inset = 14 * $s
    $arcRect = New-Object System.Drawing.RectangleF $inset, $inset, ($size - 2 * $inset), ($size - 2 * $inset)
    $track = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 58, 64, 76)), $arcWidth
    $track.StartCap = $track.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawArc($track, $arcRect, 135, 270)
    $accent = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 79, 140, 255)), $arcWidth
    $accent.StartCap = $accent.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawArc($accent, $arcRect, 135, 200)

    # Knob body with a soft vertical gradient.
    $bodyInset = 46 * $s
    $bodyRect = New-Object System.Drawing.RectangleF $bodyInset, $bodyInset, ($size - 2 * $bodyInset), ($size - 2 * $bodyInset)
    $body = New-Object System.Drawing.Drawing2D.LinearGradientBrush $bodyRect, ([System.Drawing.Color]::FromArgb(255, 72, 78, 92)), ([System.Drawing.Color]::FromArgb(255, 30, 33, 40)), 90
    $g.FillEllipse($body, $bodyRect)
    $rim = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 96, 104, 120)), ([Math]::Max(1, 3 * $s))
    $g.DrawEllipse($rim, $bodyRect)

    # Indicator: a short line pointing at the end of the accent arc (335 degrees).
    $cx = $size / 2.0; $cy = $size / 2.0
    $angle = 335 * [Math]::PI / 180
    $r1 = 38 * $s; $r2 = 70 * $s
    $mark = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([Math]::Max(1.5, 14 * $s))
    $mark.StartCap = $mark.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawLine($mark, [float]($cx + $r1 * [Math]::Cos($angle)), [float]($cy + $r1 * [Math]::Sin($angle)),
                       [float]($cx + $r2 * [Math]::Cos($angle)), [float]($cy + $r2 * [Math]::Sin($angle)))
    $g.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    return , $ms.ToArray()
}

$sizes = 16, 24, 32, 48, 64, 128, 256
$images = foreach ($size in $sizes) { , (Get-PngBytes (New-KnobBitmap $size)) }

[System.IO.File]::WriteAllBytes((Join-Path $OutDir 'icon.png'), $images[-1])

# ICO container: header, one directory entry per size, then the PNG data.
$out = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $out
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $w.Write([Byte]$dim); $w.Write([Byte]$dim); $w.Write([Byte]0); $w.Write([Byte]0)
    $w.Write([UInt16]1); $w.Write([UInt16]32)
    $w.Write([UInt32]$images[$i].Length); $w.Write([UInt32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $w.Write($img) }
$w.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $OutDir 'icon.ico'), $out.ToArray())
Write-Output "wrote icon.png and icon.ico ($($sizes -join ', ') px) to $OutDir"
