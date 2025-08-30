#!/usr/bin/env pwsh

# Quick configuration generator for infrared parameters
param(
    [Parameter(Mandatory=$false)]
    [ValidateRange(0.0, 1.0)]
    [double]$Min = 0.25,
    
    [Parameter(Mandatory=$false)]
    [ValidateRange(0.0, 1.0)]
    [double]$Max = 1.0,
    
    [Parameter(Mandatory=$false)]
    [ValidateRange(0.1, 20.0)]
    [double]$Scale = 3.0,
    
    [Parameter(Mandatory=$false)]
    [string]$Preset
)

function Set-Config {
    param($min, $max, $scale, $description)
    
    if ($min -ge $max) {
        Write-Error "Minimum value ($min) must be less than maximum value ($max)"
        return
    }
    
    $config = @{
        "infrared_output_value_minimum" = $min
        "infrared_output_value_maximum" = $max
        "infrared_source_scale" = $scale
    } | ConvertTo-Json
    
    Write-Host "Setting $description configuration:" -ForegroundColor Green
    Write-Host $config
    
    $config | Out-File -FilePath "infrared_config.json" -Encoding UTF8
    Write-Host "✅ Configuration saved to infrared_config.json" -ForegroundColor Green
}

# Handle presets
switch ($Preset) {
    "default" {
        Set-Config 0.25 1.0 3.0 "default"
    }
    "bright" {
        Set-Config 0.5 1.0 8.0 "bright"
    }
    "soft" {
        Set-Config 0.4 0.9 2.0 "soft"
    }
    "contrast" {
        Set-Config 0.1 1.0 5.0 "high contrast"
    }
    "dark" {
        Set-Config 0.05 0.7 1.5 "dark/subtle"
    }
    default {
        # Use provided parameters
        Set-Config $Min $Max $Scale "custom"
    }
}

Write-Host "`nAvailable presets:" -ForegroundColor Cyan
Write-Host "  -Preset default   # Balanced settings" -ForegroundColor Gray
Write-Host "  -Preset bright    # Very bright image" -ForegroundColor Gray
Write-Host "  -Preset soft      # Softer, lower contrast" -ForegroundColor Gray
Write-Host "  -Preset contrast  # High contrast" -ForegroundColor Gray
Write-Host "  -Preset dark      # Dark/subtle image" -ForegroundColor Gray
Write-Host "`nCustom usage:" -ForegroundColor Cyan
Write-Host "  .\set_config.ps1 -Min 0.2 -Max 0.9 -Scale 4.0" -ForegroundColor Gray
