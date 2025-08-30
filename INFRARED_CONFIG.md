# Infrared Configuration System

This document explains how to use the runtime infrared configuration system for experimenting with different parameter values.

## Overview

The infrared pipeline now supports runtime configuration changes through a JSON configuration file. The system monitors the config file every 3 seconds and automatically applies changes without restarting the application.

## Configuration File

The configuration file is `infrared_config.json` located in the project root directory. It contains three parameters that control infrared image processing:

```json
{
  "infrared_output_value_minimum": 0.25,
  "infrared_output_value_maximum": 1.0,
  "infrared_source_scale": 3.0
}
```

### Parameters

- **infrared_output_value_minimum** (0.0 - 1.0): Sets the lower brightness limit after processing. Higher values create a brighter "wall" effect, making darker areas more visible.

- **infrared_output_value_maximum** (0.0 - 1.0): Sets the upper brightness limit after processing. Must be greater than the minimum value.

- **infrared_source_scale** (> 0.0): Scaling factor applied to the raw infrared data. Higher values increase overall brightness and contrast.

## How to Use

1. Start the kinect-rtsp application as normal
2. Open your RTSP stream in VLC or another viewer
3. Edit the `infrared_config.json` file with your preferred values
4. Save the file
5. Within 3 seconds, you should see a log message indicating the config was reloaded
6. Observe the changes in the infrared video stream
7. Repeat steps 3-6 to experiment with different combinations

## Example Configurations

### Default (Balanced)
```json
{
  "infrared_output_value_minimum": 0.25,
  "infrared_output_value_maximum": 1.0,
  "infrared_source_scale": 3.0
}
```

### High Contrast
```json
{
  "infrared_output_value_minimum": 0.1,
  "infrared_output_value_maximum": 1.0,
  "infrared_source_scale": 5.0
}
```

### Softer Image
```json
{
  "infrared_output_value_minimum": 0.4,
  "infrared_output_value_maximum": 0.9,
  "infrared_source_scale": 2.0
}
```

### Very Bright
```json
{
  "infrared_output_value_minimum": 0.5,
  "infrared_output_value_maximum": 1.0,
  "infrared_source_scale": 8.0
}
```

## Log Messages

When the configuration is reloaded, you'll see messages like:
- `📄 Infrared config file changed, reloading...`
- `✅ Infrared config reloaded: min=0.25, max=1.00, scale=3.0`
- `🔄 Regenerating infrared LUT with new config values`

## Tips for Experimentation

1. Start with small changes to see their effects
2. Keep the minimum value less than the maximum value
3. Very high scale values (>10) may cause overexposure
4. Very low minimum values (<0.1) may make the image too dark
5. Use a systematic approach: change one parameter at a time to understand its individual effect
6. Take notes of configurations that work well for your specific use case

## Validation

The system validates configuration values:
- Minimum and maximum values must be between 0.0 and 1.0
- Minimum must be less than maximum
- Scale must be greater than 0.0

Invalid configurations will be rejected with an error message in the logs.
