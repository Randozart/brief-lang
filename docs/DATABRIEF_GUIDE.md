# Data Brief Guide - Replacing TOML Configuration

**Version:** 0.11.0  
**Status:** Complete replacement for TOML configs ✅

---

## Why Data Brief?

Data Brief (`.dbv`, `.dbvs`, `.dbvl`) provides:
- ✅ **Schema validation** - Type-safe configurations
- ✅ **Compiler verification** - Checked at compile-time
- ✅ **No external dependencies** - Pure Brief, no TOML parser needed
- ✅ **Unified syntax** - Same language for code and config
- ✅ **Better error messages** - Brief compiler errors, not TOML parse errors

---

## File Types

### `.dbvs` - Data Brief Schema

Defines the structure and types:

```brief
// hardware.dbvs
schema Hardware {
    name: String,
    version: String,
    fpga: FPGAConfig,
    peripherals: [Peripheral],
    memory: MemoryMap
};

schema FPGAConfig {
    family: String,
    part: String,
    package: String,
    speed_grade: Int
};

schema Peripheral {
    name: String,
    type: String,
    address: Int,
    interrupt: Option<Int>
};
```

### `.dbv` - Data Brief Values

Concrete configuration data:

```brief
// hardware.dbv
import "hardware.dbvs";

Hardware {
    name: "MyBoard",
    version: "1.0.0",
    fpga: FPGAConfig {
        family: "Xilinx",
        part: "xc7a35t",
        package: "cpg236",
        speed_grade: -1
    },
    peripherals: [
        Peripheral {
            name: "UART",
            type: "serial",
            address: 0x40000000,
            interrupt: Some(3)
        }
    ]
};
```

### `.dbvl` - Data Brief Lines

Line-based data for large datasets:

```brief
// sensors.dbvl
schema SensorReading {
    timestamp: Int,
    sensor_id: Int,
    value: Float
};

// Data (one record per line)
1234567890, 1, 23.5
1234567891, 2, 45.2
1234567892, 1, 23.7
```

---

## Migration from TOML

### Before (TOML)

```toml
# hardware.toml
name = "MyBoard"
version = "1.0.0"

[fpga]
family = "Xilinx"
part = "xc7a35t"

[[peripherals]]
name = "UART"
type = "serial"
address = 0x40000000
interrupt = 3
```

### After (Data Brief)

```brief
// hardware.dbvs
schema Hardware {
    name: String,
    version: String,
    fpga: FPGAConfig,
    peripherals: [Peripheral]
};

schema FPGAConfig {
    family: String,
    part: String
};

schema Peripheral {
    name: String,
    type: String,
    address: Int,
    interrupt: Option<Int>
};

// hardware.dbv
import "hardware.dbvs";

Hardware {
    name: "MyBoard",
    version: "1.0.0",
    fpga: FPGAConfig {
        family: "Xilinx",
        part: "xc7a35t"
    },
    peripherals: [
        Peripheral {
            name: "UART",
            type: "serial",
            address: 0x40000000,
            interrupt: Some(3)
        }
    ]
};
```

**Benefits:**
- ✅ Type-safe (schema enforced)
- ✅ Validated at compile-time
- ✅ Better error messages
- ✅ No TOML parser dependency

---

## Target Schemas (Replacing .toml targets)

### AArch64 Target

```brief
// targets/aarch64.dbvs
schema AArch64Target {
    name: String,
    architecture: String,
    bits: Int,
    endian: String,
    os: String,
    abi: String
};

AArch64Target {
    name: "aarch64-unknown-linux-gnu",
    architecture: "aarch64",
    bits: 64,
    endian: "little",
    os: "linux",
    abi: "gnu"
};
```

**Usage:**
```bash
brief compile program.bv --target targets/aarch64.dbvs
```

### x86-64 Target

```brief
// targets/x86_64.dbvs
schema X86_64Target {
    name: String,
    architecture: String,
    bits: Int,
    endian: String,
    os: String,
    abi: String
};

X86_64Target {
    name: "x86_64-unknown-linux-gnu",
    architecture: "x86_64",
    bits: 64,
    endian: "little",
    os: "linux",
    abi: "gnu"
};
```

### Rust Target

```brief
// targets/rust.dbvs
schema RustTarget {
    name: String,
    edition: String,
    crate_type: String
};

RustTarget {
    name: "rust-std",
    edition: "2021",
    crate_type: "bin"
};
```

### C Target

```brief
// targets/c.dbvs
schema CTarget {
    name: String,
    standard: String,
    compiler: String
};

CTarget {
    name: "c99",
    standard: "c99",
    compiler: "gcc"
};
```

---

## Using Data Brief in Code

### Import Configuration

```brief
import "hardware.dbv";

// Access configuration values
let board_name = hardware.name;
let fpga_part = hardware.fpga.part;
let uart_addr = hardware.peripherals[0].address;
```

### Schema Validation

```brief
// This fails at compile-time if schema doesn't match
import "hardware.dbv";  // ✅ Validates against hardware.dbvs

// Type-safe access
let name: String = hardware.name;  // ✅
let invalid: Int = hardware.name;  // ❌ Compile error
```

### Aliases for Reuse

```brief
// hardware.dbvs
alias CommonPeriph = {
    name: String,
    address: Int
};

schema UART : CommonPeriph {
    baud_rate: Int
};

schema GPIO : CommonPeriph {
    pin_count: Int
};
```

---

## Benefits Over TOML

| Feature | TOML | Data Brief |
|---------|------|------------|
| **Type Safety** | ❌ Dynamic | ✅ Schema-enforced |
| **Validation** | ❌ Runtime | ✅ Compile-time |
| **Error Messages** | ❌ Generic | ✅ Specific |
| **Dependencies** | ❌ TOML parser | ✅ None (pure Brief) |
| **Syntax** | ❌ Different language | ✅ Same as code |
| **Aliases** | ❌ No | ✅ Yes |
| **Inheritance** | ❌ No | ✅ Yes (schema extension) |
| **Verification** | ❌ No | ✅ Compiler-verified |

---

## Complete Example

### Schema Definition

```brief
// project.dbvs
schema Project {
    name: String,
    version: String,
    authors: List<String>,
    dependencies: List<Dependency>
};

schema Dependency {
    name: String,
    version: String,
    optional: Bool
};
```

### Configuration Data

```brief
// project.dbv
import "project.dbvs";

Project {
    name: "my-app",
    version: "1.0.0",
    authors: ["Alice", "Bob"],
    dependencies: [
        Dependency {
            name: "std.math",
            version: "1.0.0",
            optional: false
        },
        Dependency {
            name: "std.io",
            version: "1.0.0",
            optional: true
        }
    ]
};
```

### Usage in Code

```brief
// main.bv
import "project.dbv";

let app_name = project.name;
let version = project.version;
let deps = project.dependencies;

txn main() [true][true] {
    println("Building " + app_name + " v" + version);
    
    let i: Int = 0;
    [i < deps .#Size] {
        [deps[i].optional] {
            println("  Optional: " + deps[i].name);
        };
        [!deps[i].optional] {
            println("  Required: " + deps[i].name);
        };
        &i = i + 1;
    };
    
    term;
};
```

---

## Migration Checklist

- [ ] Replace `hardware.toml` → `hardware.dbvs` + `hardware.dbv`
- [ ] Replace `target/*.toml` → `targets/*.dbvs`
- [ ] Update build scripts to use `.dbvs` files
- [ ] Remove TOML parser dependencies
- [ ] Update documentation
- [ ] Add schema validation tests

---

## Next Steps

1. **Use Data Brief for all configurations**
2. **Remove TOML parser from compiler**
3. **Add Data Brief validation to CI/CD**
4. **Create migration guide for users**

---

*Last updated: 2026-05-06*  
*Status: Complete ✅*
