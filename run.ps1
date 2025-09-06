param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

# Use the GSTREAMER_1_0_ROOT_MSVC_X86_64 environment variable if present
$root = $env:GSTREAMER_1_0_ROOT_MSVC_X86_64
if ($null -ne $root -and (Test-Path $root)) {
    $bin = Join-Path $root "bin"
    $plugins = Join-Path $root "lib\gstreamer-1.0"
    if ((Test-Path $bin) -and (Test-Path $plugins)) {
        $env:PATH = "$bin;$env:PATH"
        $env:GST_PLUGIN_PATH = $plugins
        Write-Host "Using GStreamer runtime from $root"
    } else {
        Write-Warning "GStreamer folders not found: $bin , $plugins"
    }
} else {
    Write-Warning "GSTREAMER_1_0_ROOT_MSVC_X86_64 not set or not found, make sure you have downloaded and installed GStreamer runtime from https://gstreamer.freedesktop.org/download/#windows."
}

# Locate the exe next to the script (the script will be copied into the same folder as the exe by build.rs)
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$exe = Join-Path $scriptDir "kinect-rtsp.exe"
if (-not (Test-Path $exe)) {
    # Fallback: look for exe in current working directory
    $exe = Join-Path (Get-Location) "kinect-rtsp.exe"
}

if (-not (Test-Path $exe)) {
    Write-Error "Cannot find kinect-rtsp.exe near $scriptDir or current directory."
    exit 1
}

# Execute the real binary and forward arguments
& $exe @Args
exit $LASTEXITCODE
