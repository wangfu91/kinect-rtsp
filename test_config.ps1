#!/usr/bin/env pwsh

# Test script for infrared configuration validation

Write-Host "Testing infrared configuration system..." -ForegroundColor Green

# Test valid configuration
$validConfig = @{
    "infrared_output_value_minimum" = 0.3
    "infrared_output_value_maximum" = 0.95
    "infrared_source_scale" = 4.5
} | ConvertTo-Json

Write-Host "`nTesting valid configuration:" -ForegroundColor Yellow
Write-Host $validConfig

$validConfig | Out-File -FilePath "infrared_config.json" -Encoding UTF8
Write-Host "✅ Valid config saved to infrared_config.json" -ForegroundColor Green

# Wait a moment then test another configuration
Start-Sleep -Seconds 2

$testConfig2 = @{
    "infrared_output_value_minimum" = 0.1
    "infrared_output_value_maximum" = 1.0
    "infrared_source_scale" = 6.0
} | ConvertTo-Json

Write-Host "`nTesting high contrast configuration:" -ForegroundColor Yellow
Write-Host $testConfig2

$testConfig2 | Out-File -FilePath "infrared_config.json" -Encoding UTF8
Write-Host "✅ High contrast config saved to infrared_config.json" -ForegroundColor Green

Write-Host "`nYou can now run the kinect-rtsp application and modify infrared_config.json" -ForegroundColor Cyan
Write-Host "The changes will be applied automatically every 3 seconds." -ForegroundColor Cyan
