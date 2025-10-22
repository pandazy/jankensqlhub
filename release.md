# Release Notes v0.6.1

## ✨ **New Feature: Conditional Enum Constraints (`enumif`)**

### Conditional Enum Constraint Support
- **NEW**: Added `enumif` constraint for conditional enum validation
- **Purpose**: Enables parameter validation where allowed values depend on other parameter values
- **Conditional Parameters**: Can reference any primitive type (string, number, boolean), not just enum values
- **Multiple Conditions**: If multiple conditional parameters are specified, they're evaluated alphabetically

### Syntax
```json
{
  "parameter_name": {
    "enumif": {
      "conditional_param": {
        "condition_value1": ["allowed1", "allowed2"],
        "condition_value2": ["different1", "different2"]
      }
    }
  }
}
```

### Examples
```json
{
  "media_source": {
    "enumif": {
      "media_type": {
        "song": ["artist", "album", "title"],
        "show": ["channel", "category", "episodes"]
      }
    }
  },
  "priority": {
    "enumif": {
      "severity": {
        "high": ["urgent", "immediate"],
        "low": ["optional", "backlog"]
      }
    }
  }
}
```

### Validation Behavior
- Conditional parameter value must match a defined condition
- Parameter value must be in the allowed array for the matching condition
- Multiple conditional parameters evaluated alphabetically (first match wins)

---
**Version 0.6.1** - Added conditional enum constraints (`enumif`)
