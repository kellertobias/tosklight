# Locate an existing Node runtime; install nothing and preserve the caller's environment.
$driverNodeCommand = Get-Command node -ErrorAction SilentlyContinue
$driverNodeCandidates = @()
if ($driverNodeCommand) { $driverNodeCandidates += $driverNodeCommand.Source }
$driverBundledRoot = Join-Path $env:LOCALAPPDATA 'OpenAI\Codex\runtimes\cua_node'
if (Test-Path -LiteralPath $driverBundledRoot) {
    $driverNodeCandidates += Get-ChildItem -LiteralPath $driverBundledRoot -Directory | Sort-Object LastWriteTime -Descending | ForEach-Object { Join-Path $_.FullName 'bin\node.exe' }
}
foreach ($driverCandidate in $driverNodeCandidates) {
    if (-not (Test-Path -LiteralPath $driverCandidate)) { continue }
    $driverVersion = & $driverCandidate -p 'parseInt(process.versions.node)'
    if ($LASTEXITCODE -eq 0 -and [int]$driverVersion -ge 22) { $driverCandidate; return }
}
throw 'Install Node.js 22 or newer, then start the hardware panel again.'
