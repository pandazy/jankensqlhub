# Release Notes v0.9.0

## ✨ **Enhanced Error Handling**

### Flexible Error Processing
- **Unique Error Codes**: Each error type has a unique u16 identifier (range: 2000-2030) for programmatic identification
- **Structured JSON Metadata**: Rich contextual information stored as JSON strings in all error variants
- **Extract Functions**: `get_error_data()` gets ErrorData from any JankenError variant
- **Info Lookup**: `get_error_info(code)` provides comprehensive error descriptions and categories
- **Metadata Constants**: `M_EXPECTED`, `M_GOT`, `M_PARAM_NAME`, etc. for consistent field access
- **Helper Function**: `error_meta()` extracts metadata fields without parsing JSON

### Error Architecture
```rust
use jankensqlhub::{JankenError, get_error_data, get_error_info, error_meta, M_EXPECTED, M_GOT};

// Extract error data from any JankenError variant
let data = get_error_data(&error);
let code = data.code;

// Look up comprehensive error information
if let Some(info) = get_error_info(code) {
    eprintln!("{}: {}", info.name, info.description);
}

// Extract specific metadata fields using constants and helper functions
let expected = error_meta(data, M_EXPECTED)?;
let got = error_meta(data, M_GOT)?;
eprintln!("Type mismatch: expected {}, got {}", expected, got);
```

### Error Categories & Codes
| Category | Codes | Description |
|----------|-------|-------------|
| Query | 2000 | Query Not Found (2000) |
| Parameter | 2010-2030 | Not Provided (2010), Type Mismatch (2020), Name Conflict (2030) |

### Benefits for Applications
- **Programmatic Error Identification**: Use error codes for switch/case logic and custom handling strategies
- **Rich Contextual Information**: JSON metadata provides specific details (expected types, actual values, parameter names, etc.)
- **Convenient Metadata Extraction**: Extract metadata fields directly with helpers and constants (balances user convenience with flexible maintenance)
- **Consistent API**: All error variants follow the same data structure across the library
- **Descriptive Information**: Look up human-readable error descriptions and categories by code

---
**Version 0.9.0** - Enhanced error handling with flexible error codes and JSON metadata
