$ErrorActionPreference='Stop'
if (Get-Process -Name 'light-desktop' -ErrorAction SilentlyContinue) { return }
$toskControlCandidates=@(
    (Join-Path $env:LOCALAPPDATA 'ToskLight\light-desktop.exe'),
    (Join-Path $env:LOCALAPPDATA 'Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\ToskLight\light-desktop.exe')
)
$toskControlExe=$toskControlCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $toskControlExe) { throw 'ToskLight Control is missing from its installed location. Install Control before starting hardware capture.' }
# This is the interactive Control window the operator needs, not a background helper.
Start-Process -FilePath $toskControlExe -WorkingDirectory (Split-Path $toskControlExe -Parent)
