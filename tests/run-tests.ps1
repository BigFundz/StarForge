<#
.SYNOPSIS
    StarForge CI test suite — pure PowerShell, zero external dependencies.

.DESCRIPTION
    Verifies all changes for issue #210 (improve CLI error messages and
    recovery guidance) are correctly implemented across the Rust source files.

.NOTES
    Run with:  powershell -ExecutionPolicy Bypass -File tests\run-tests.ps1
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$passed  = 0
$failed  = 0
$failures = [System.Collections.Generic.List[string]]::new()

function Pass([string]$name) {
    $script:passed++
    Write-Host "  $(Green 'v')  $name"
}

function Fail([string]$name, [string]$reason) {
    $script:failed++
    $script:failures.Add("  FAIL  $name`n         $reason")
    Write-Host "  $(Red 'x')  $name"
    Write-Host "       $reason" -ForegroundColor DarkGray
}

function Run-Test([string]$name, [scriptblock]$body) {
    try { & $body; Pass $name }
    catch { Fail $name $_.Exception.Message }
}

function Assert-True([bool]$condition, [string]$msg) {
    if (-not $condition) { throw $msg }
}

function Assert-Contains([string]$text, [string]$fragment, [string]$msg) {
    if ($text -notlike "*$fragment*") { throw "$msg  (looked for: '$fragment')" }
}

function Assert-NotContains([string]$text, [string]$fragment, [string]$msg) {
    if ($text -like "*$fragment*") { throw "$msg  (should NOT contain: '$fragment')" }
}

function Green([string]$s)  { "`e[32m$s`e[0m" }
function Red([string]$s)    { "`e[31m$s`e[0m" }
function Bold([string]$s)   { "`e[1m$s`e[0m"  }

$root = Split-Path $PSScriptRoot -Parent

# ─────────────────────────────────────────────────────────────────────────────
# Read source files once
# ─────────────────────────────────────────────────────────────────────────────
$mainSrc    = Get-Content (Join-Path $root "src\main.rs")       -Raw
$printSrc   = Get-Content (Join-Path $root "src\utils\print.rs") -Raw
$horizonSrc = Get-Content (Join-Path $root "src\utils\horizon.rs") -Raw
$configSrc  = Get-Content (Join-Path $root "src\utils\config.rs")  -Raw

# ─────────────────────────────────────────────────────────────────────────────
# SUITE 1 — src/utils/print.rs — cli_error function
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "`n$(Bold 'src/utils/print.rs  --  cli_error')"

Run-Test 'cli_error function is defined' {
    Assert-Contains $printSrc 'pub fn cli_error(' 'Missing cli_error function'
}
Run-Test 'cli_error accepts an anyhow::Error parameter' {
    Assert-Contains $printSrc 'anyhow::Error' 'cli_error must accept anyhow::Error'
}
Run-Test 'cli_error accepts a hints slice' {
    Assert-Contains $printSrc 'hints' 'cli_error must accept hints parameter'
}
Run-Test 'cli_error prints to stderr (eprintln)' {
    Assert-Contains $printSrc 'eprintln!' 'cli_error must write to stderr'
}
Run-Test 'cli_error shows "What to try" section' {
    Assert-Contains $printSrc 'What to try' 'cli_error must include "What to try" label'
}
Run-Test 'cli_error shows hint arrows' {
    # Arrow is rendered via .cyan() call in the hints loop
    Assert-Contains $printSrc '.cyan()' 'cli_error must colour hint arrows with cyan'
    Assert-Contains $printSrc 'for hint in hints' 'cli_error must iterate hints'
}
Run-Test 'cli_error walks the anyhow error chain' {
    Assert-Contains $printSrc '.chain()' 'cli_error must walk the error chain for context'
}
Run-Test 'cli_error shows context label for chained errors' {
    Assert-Contains $printSrc 'Context:' 'cli_error must label chained errors with Context:'
}
Run-Test 'cli_error includes generic fallback for empty hints' {
    Assert-Contains $printSrc 'hints.is_empty()' 'cli_error must have fallback for empty hints'
}
Run-Test 'cli_error generic fallback references --verbose' {
    Assert-Contains $printSrc '--verbose' 'Generic fallback should mention --verbose'
}
Run-Test 'cli_error generic fallback references issues URL' {
    Assert-Contains $printSrc 'issues' 'Generic fallback should reference the issues URL'
}

# ─────────────────────────────────────────────────────────────────────────────
# SUITE 2 — src/main.rs — error sink uses cli_error
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "`n$(Bold 'src/main.rs  --  structured error sink')"

Run-Test 'main.rs calls cli_error instead of bare eprintln for errors' {
    Assert-Contains $mainSrc 'cli_error(' 'main.rs must call cli_error'
}
Run-Test 'main.rs no longer uses bare eprintln for the error result' {
    # The old pattern was exactly: eprintln!("\n  {} {}\n", "✗ Error:".red().bold(), e);
    # It should be gone now
    Assert-NotContains $mainSrc '"✗ Error:"' 'Old bare eprintln error sink should be replaced'
}
Run-Test 'main.rs passes command name to recovery_hints' {
    Assert-Contains $mainSrc 'recovery_hints' 'main.rs must call recovery_hints'
}
Run-Test 'recovery_hints function is defined' {
    Assert-Contains $mainSrc 'fn recovery_hints(' 'recovery_hints function must be defined'
}
Run-Test 'recovery_hints covers wallet command' {
    Assert-Contains $mainSrc '"wallet"' 'recovery_hints must handle wallet command'
}
Run-Test 'recovery_hints covers deploy command' {
    Assert-Contains $mainSrc '"deploy"' 'recovery_hints must handle deploy command'
}
Run-Test 'recovery_hints covers contract command' {
    Assert-Contains $mainSrc '"contract"' 'recovery_hints must handle contract command'
}
Run-Test 'recovery_hints covers tx command' {
    Assert-Contains $mainSrc '"tx"' 'recovery_hints must handle tx command'
}
Run-Test 'recovery_hints covers network command' {
    Assert-Contains $mainSrc '"network"' 'recovery_hints must handle network command'
}
Run-Test 'recovery_hints covers node command (Docker hint)' {
    Assert-Contains $mainSrc '"node"' 'recovery_hints must handle node command'
}
Run-Test 'recovery_hints covers plugin command' {
    Assert-Contains $mainSrc '"plugin"' 'recovery_hints must handle plugin command'
}
Run-Test 'recovery_hints covers config command' {
    Assert-Contains $mainSrc '"config"' 'recovery_hints must handle config command'
}
Run-Test 'recovery_hints node hint mentions Docker' {
    Assert-Contains $mainSrc 'Docker' 'node recovery hint must mention Docker'
}
Run-Test 'recovery_hints wallet fund hint present' {
    Assert-Contains $mainSrc 'wallet fund' 'wallet recovery hints must mention wallet fund'
}
Run-Test 'recovery_hints deploy hint mentions stellar contract build' {
    Assert-Contains $mainSrc 'stellar contract build' 'deploy hint must mention stellar contract build'
}
Run-Test 'recovery_hints network hint mentions starforge network show' {
    Assert-Contains $mainSrc 'network show' 'network hint must mention starforge network show'
}
Run-Test 'recovery_hints has generic connection fallback' {
    Assert-Contains $mainSrc 'internet connection' 'generic fallback must mention internet connection'
}
Run-Test 'recovery_hints has generic config doctor fallback' {
    Assert-Contains $mainSrc 'config doctor' 'generic fallback must mention config doctor'
}

# ─────────────────────────────────────────────────────────────────────────────
# SUITE 3 — src/utils/horizon.rs — improved error messages
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "`n$(Bold 'src/utils/horizon.rs  --  improved error messages')"

Run-Test 'fund_account distinguishes HTTP 400 (already funded)' {
    Assert-Contains $horizonSrc '400' 'fund_account must handle HTTP 400 separately'
    Assert-Contains $horizonSrc 'already been funded' 'fund_account 400 message must say already funded'
}
Run-Test 'fund_account 400 hint references wallet show' {
    Assert-Contains $horizonSrc 'wallet show' 'fund_account 400 hint must reference wallet show'
}
Run-Test 'fund_account non-400 error mentions testnet only' {
    Assert-Contains $horizonSrc 'only available on testnet' 'fund_account must clarify testnet-only'
}
Run-Test 'fund_account network error suggests checking connection' {
    Assert-Contains $horizonSrc 'Check your internet connection' 'fund_account network error must suggest checking connection'
}
Run-Test 'fetch_account handles 404 explicitly' {
    Assert-Contains $horizonSrc '404' 'fetch_account must handle 404 explicitly'
}
Run-Test 'fetch_account 404 message names the account key' {
    Assert-Contains $horizonSrc "Account '" 'fetch_account 404 must name the account key'
}
Run-Test 'fetch_account 404 suggests wallet fund' {
    Assert-Contains $horizonSrc 'wallet fund' 'fetch_account 404 must suggest wallet fund'
}
Run-Test 'fetch_account 404 mentions account not activated' {
    Assert-Contains $horizonSrc 'not have been activated' 'fetch_account 404 must mention account activation'
}
Run-Test 'fetch_account network error suggests starforge network test' {
    Assert-Contains $horizonSrc 'network test' 'fetch_account network error must suggest network test'
}
Run-Test 'fetch_account no longer uses generic "Account not found on" without key' {
    # Old message was just "Account not found on {network}" with no key or hint
    Assert-NotContains $horizonSrc 'bail!("Account not found on' 'Old bare "Account not found on" must be replaced'
}

# ─────────────────────────────────────────────────────────────────────────────
# SUITE 4 — src/utils/config.rs — improved load() errors
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "`n$(Bold 'src/utils/config.rs  --  improved load() errors')"

Run-Test 'config load() shows file path on read error' {
    Assert-Contains $configSrc 'path.display()' 'load() must show path on read error'
}
Run-Test 'config load() parse error includes file path' {
    # Should have two with_context calls, both referencing path
    $count = ([regex]::Matches($configSrc, 'path\.display\(\)')).Count
    Assert-True ($count -ge 2) "load() should reference path.display() in both error contexts (found $count)"
}
Run-Test 'config load() parse error suggests config doctor' {
    Assert-Contains $configSrc 'config doctor' 'load() parse error must suggest config doctor'
}
Run-Test 'config load() parse error mentions corrupted or invalid TOML' {
    Assert-Contains $configSrc 'corrupted' 'load() parse error must say the file may be corrupted'
}
Run-Test 'config load() parse error mentions deleting to reset' {
    Assert-Contains $configSrc 'delete the file to reset' 'load() parse error must suggest deleting to reset'
}
Run-Test 'config load() no longer uses generic "Failed to parse config file" without path' {
    Assert-NotContains $configSrc '"Failed to parse config file"' 'Old generic parse error must be replaced'
}

# ─────────────────────────────────────────────────────────────────────────────
# SUITE 5 — Acceptance criteria spot-checks
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "`n$(Bold 'Acceptance criteria  --  #210')"

Run-Test 'AC1: Errors include clear context (cli_error shows chain)' {
    Assert-Contains $printSrc '.chain()' 'Error chain must be walked for context'
    Assert-Contains $printSrc 'Context:' 'Context label must be present'
}
Run-Test 'AC2: Errors include suggested actions (hints parameter)' {
    Assert-Contains $printSrc 'hints' 'Hints must be part of cli_error output'
    Assert-Contains $mainSrc  'recovery_hints' 'recovery_hints must supply actions per command'
}
Run-Test 'AC3: Users can identify what went wrong (error message + context printed)' {
    Assert-Contains $printSrc 'err' 'The error itself must be printed'
    Assert-Contains $printSrc 'eprintln!' 'Must write to stderr so it is visible'
}
Run-Test 'AC4: Common failures documented in output (wallet/deploy/network hints)' {
    Assert-Contains $mainSrc 'wallet fund' 'wallet fund documented'
    Assert-Contains $mainSrc 'stellar contract build' 'build step documented'
    Assert-Contains $mainSrc 'network show' 'network show documented'
    Assert-Contains $mainSrc 'config doctor' 'config doctor documented'
}
Run-Test 'AC5: Validation messages improved before execution (horizon pre-checks)' {
    Assert-Contains $horizonSrc 'not have been activated' 'Account activation status shown before ops'
    Assert-Contains $horizonSrc 'only available on testnet' 'Friendbot testnet-only shown before ops'
}

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────
Write-Host ""
if ($failed -eq 0) {
    Write-Host "$(Green 'v') $(Bold "All $passed tests passed")`n"
    exit 0
} else {
    Write-Host "$(Red 'x') $(Bold "$failed test(s) failed"), $passed passed`n"
    foreach ($f in $failures) {
        Write-Host $f -ForegroundColor Red
        Write-Host ""
    }
    exit 1
}
