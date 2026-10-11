# Crewboard local checks on Windows: fmt, clippy, tests. The same three steps as scripts/check.sh, in the same
# order (decision 0056, SDD section 23.2).
#
# Run this before every commit made from a Windows machine. Works from any folder inside the repository. Stops at
# the first failing step and exits non-zero. Setup of a Windows machine or VM: scripts/README.md.
#
#   powershell -ExecutionPolicy Bypass -File scripts\check.ps1

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Error 'check.ps1: git is required (core tests run against temporary git repositories)'
    exit 1
}

$root = git -C $PSScriptRoot rev-parse --show-toplevel 2>$null
if ($LASTEXITCODE -ne 0 -or -not $root) {
    Write-Error 'check.ps1: not inside the Crewboard repository'
    exit 1
}
Set-Location $root

# The tests run ordinary Unix commands (sh, cat) in place of claude, as on macOS and Linux (SDD section 23.1). Git for
# Windows ships them in usr\bin; they go at the end of PATH, so Windows' own commands keep precedence.
$gitRoot = Split-Path (Split-Path (Get-Command git).Source)
$usrBin = Join-Path $gitRoot 'usr\bin'
if (-not (Test-Path (Join-Path $usrBin 'sh.exe'))) {
    Write-Error "check.ps1: Git for Windows' sh.exe wasn't found in $usrBin"
    exit 1
}
$env:PATH = "$env:PATH;$usrBin"

function Invoke-Step([string] $Name, [string[]] $Command) {
    Write-Host "==> ${Name}: $($Command -join ' ')"
    & $Command[0] $Command[1..($Command.Length - 1)]
    if ($LASTEXITCODE -ne 0) {
        Write-Host "check.ps1: FAILED at step '$Name'"
        exit 1
    }
}

Invoke-Step 'fmt' @('cargo', 'fmt', '--all', '--check')
Invoke-Step 'clippy' @('cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
Invoke-Step 'test' @('cargo', 'test', '--workspace')

Write-Host 'check.ps1: all checks passed'
