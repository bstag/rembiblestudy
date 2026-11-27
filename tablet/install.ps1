#requires -Version 7.0
[CmdletBinding()]
param(
    [string]$Binary = (Join-Path $PSScriptRoot '../target/tablet/aarch64-unknown-linux-gnu/release/rembiblestudy'),
    [string]$Patch = (Join-Path $PSScriptRoot 'rembiblestudy-selection.qmd'),
    [string]$SshKey = (Join-Path $env:USERPROFILE '.ssh/id_rsa'),
    [string]$ApiKeyFile,
    [switch]$PromptApiKey,
    [switch]$Reload
)

$ErrorActionPreference = 'Stop'
if ($PromptApiKey -and $ApiKeyFile) {
    throw 'Choose either -PromptApiKey or -ApiKeyFile.'
}
$device = 'root@10.11.99.1'
$sshOptions = @('-o', 'StrictHostKeyChecking=yes', '-o', 'ConnectTimeout=5', '-i', $SshKey)
$patchPath = (Resolve-Path -LiteralPath $Patch).Path
$patchSource = [IO.File]::ReadAllText($patchPath).Replace("`r`n", "`n")
$patchHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
    [Text.Encoding]::UTF8.GetBytes($patchSource)
))
if ($patchHash -eq 'C7DB8069F1824DAC4809BD3193743DEDD04CBC7A0EE85A6E7CF51BE0330D8130') {
    throw 'This preview patch caused a startup loop on Paper Pro firmware 3.29.1.0.'
}
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$stage = '/home/root/xovi/exthome/rembiblestudy/stage-' + [Guid]::NewGuid().ToString('N')

function Invoke-Tablet([string]$Command) {
    & ssh -T @sshOptions $device $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Tablet command failed with exit code $LASTEXITCODE"
    }
}

$check = @'
set -eu
. /etc/os-release
test "$IMG_VERSION" = 3.29.1.0
test "$(tr -d '\000' < /sys/firmware/devicetree/base/model)" = "reMarkable Ferrari"
test -f /home/root/xovi/extensions.d/qt-command-executor.so
test -f /home/root/xovi/extensions.d/qt-resource-rebuilder.so
test -s /home/root/xovi/exthome/qt-resource-rebuilder/hashtab
command -v flock >/dev/null
'@
Invoke-Tablet ($check + "`nmkdir -p -m 700 $stage")

try {
    & scp -O @sshOptions $binaryPath "${device}:${stage}/entry"
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not copy the ARM64 backend'
    }
    & scp -O @sshOptions $patchPath "${device}:${stage}/rembiblestudy-selection.qmd"
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not copy the selection patch'
    }

    Invoke-Tablet "chmod 0755 $stage/entry && $stage/entry --help >/dev/null && grep -Fxq 'VERSION 3.29.1.0' $stage/rembiblestudy-selection.qmd"

    if ($Reload -and ($PromptApiKey -or $ApiKeyFile)) {
        $provision = @'
set -eu
umask 077
dir=/home/root/.config/rembiblestudy
mkdir -p "$dir"
chmod 0700 "$dir"
tmp=$(mktemp "$dir/.api-key.XXXXXX")
trap 'rm -f "$tmp"' EXIT
cat > "$tmp"
test -s "$tmp"
mv "$tmp" "$dir/api-key"
'@
        try {
            $apiKey = if ($PromptApiKey) {
                (Read-Host 'API.Bible key' -MaskInput).Trim()
            } else {
                [IO.File]::ReadAllText((Resolve-Path -LiteralPath $ApiKeyFile).Path).Trim()
            }
            if (-not $apiKey -or $apiKey -match '[\r\n]') {
                throw 'Enter one nonempty API.Bible key.'
            }
            $apiKey | & ssh -T @sshOptions $device $provision
            if ($LASTEXITCODE -ne 0) {
                throw 'Could not provision the API.Bible key'
            }
        } finally {
            Remove-Variable apiKey -ErrorAction SilentlyContinue
        }
    }

    if (!$Reload) {
        Write-Output 'Tablet and release files passed preflight. Run again with -Reload to install.'
        return
    }
    & scp -O @sshOptions (Join-Path $PSScriptRoot 'preview-trial.sh') "${device}:${stage}/"
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not copy the guarded activation script'
    }
    $activate = @'
set -eu
sh -n STAGE/preview-trial.sh
sh STAGE/preview-trial.sh begin STAGE
sh /run/rembiblestudy-preview/preview-trial.sh verify
'@
    Invoke-Tablet ($activate.Replace('STAGE', $stage))
} finally {
    Invoke-Tablet "rm -f $stage/entry $stage/rembiblestudy-selection.qmd $stage/preview-trial.sh && rmdir $stage"
}
