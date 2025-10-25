# Release Notes v1.0.0

## 🎉 **Stable Release: Feature Flags & Production Ready**

### Feature Flags for Backend Selection
- **Flexible Backend Selection**: Optional feature flags to include only needed database backends
- **`all`** (default): Enable both SQLite and PostgreSQL support
- **`sqlite`**: SQLite support only - reduces dependency footprint
- **`postgresql`**: PostgreSQL support only - reduces dependency footprint
- **Conditional Compilation**: Automatic module inclusion based on enabled features

### Installation Examples
```bash
# Default: both backends (recommended for development)
cargo add jankensqlhub

# Production: choose specific backend
cargo add jankensqlhub --features sqlite      # SQLite only
cargo add jankensqlhub --features postgresql # PostgreSQL only
```

### Benefits
- **Reduced Dependencies**: Only compile the database libraries you need
- **Smaller Binaries**: Feature-gated dependencies reduce binary size
- **Deployment Flexibility**: Deploy with only required database support
- **Backward Compatible**: Default behavior unchanged (both backends enabled)

---

**Version 1.0.0** - Production-ready with feature flags for flexible backend selection
