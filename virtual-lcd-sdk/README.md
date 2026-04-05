# virtual-lcd-sdk

`virtual-lcd-sdk` provides the shared types and traits used by the Virtual LCD Firmware Simulator crates.

It includes:

- `Color` with RGB helpers and RGB565 conversion utilities
- `Lcd` for high-level display operations
- `LcdBus` for lower-level command/data transport
- `PinId` for common LCD control pins

```toml
[dependencies]
virtual-lcd-sdk = "0.1"
```

Repository: <https://github.com/fhfelipefh/Virtual-LCD-Firmware-Simulator>
