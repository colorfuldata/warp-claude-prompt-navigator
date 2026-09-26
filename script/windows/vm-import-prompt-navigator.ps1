param(
    [string]$Destination = 'D:\WarpPromptNavigator'
)

$ErrorActionPreference = 'Stop'
$archive = Join-Path $PSScriptRoot 'warp-prompt-navigator-source.tar.gz'
if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
    throw "Source archive is missing: $archive"
}

$drive = Get-PSDrive -Name D -ErrorAction Stop
if ($drive.Free -lt 30GB) {
    throw "D: needs at least 30 GB of free space; found $([math]::Round($drive.Free / 1GB, 1)) GB"
}

$source = Join-Path $Destination 'source'
New-Item -ItemType Directory -Path $source -Force | Out-Null
& tar.exe -xzf $archive -C $source --exclude='./.claude/skills'
if ($LASTEXITCODE -ne 0) {
    throw 'Could not extract Warp source archive'
}

Write-Output "Imported Warp source to $source"
Write-Output "D: free space: $([math]::Round((Get-PSDrive D).Free / 1GB, 1)) GB"
Write-Output "CPU: $env:PROCESSOR_ARCHITECTURE"
foreach ($tool in @('git', 'cargo', 'winget', 'cmake', 'protoc', 'ISCC')) {
    $command = Get-Command $tool -ErrorAction SilentlyContinue
    Write-Output "$tool : $($command.Source)"
}
