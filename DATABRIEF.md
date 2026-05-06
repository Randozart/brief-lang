# Data Brief: Configuration & Schemas

Data Brief is a suite of formats used for configuration, schema definition, and line-based databases, replacing the need for TOML, YAML, or JSON in many parts of the Brief ecosystem.

## File Types

| Extension | Name | Purpose |
|---|---|---|
| `.dbv` | Data Brief Volume | A configuration file, similar to `hardware.toml`, that defines hardware layouts, memory maps, and other project-specific settings. It **must** reference a `.dbvs` schema. |
| `.dbvs` | Data Brief Schema | A schema definition file. It defines templates for hardware registers, memory structures, and aliases. This is where you define the "shape" of your hardware. |
| `.dbvl` | Data Brief Volume (Line) | A line-based, mutable database format used for storing records. |

## Workflow

1.  **Define a Schema (`.dbvs`)**: You create a `.dbvs` file to define the hardware components and their memory layouts. This includes registers, structs, and aliases.

    ```brief
    // file: my_device.dbvs
    ALIAS RAM: 0x40000000;
    ALIAS UART: 0x80000000;
    ```

2.  **Create a Configuration (`.dbv`)**: You create a `.dbv` file that imports the schema and provides concrete values for your specific hardware setup.

    ```brief
    // file: my_board.dbv
    IMPORT "my_device.dbvs";
    ```

3.  **Use in Brief Code (`.ebv`)**: In your embedded Brief code, you can now use the aliases defined in the schema.

    ```brief
    // file: main.ebv
    IMPORT "my_board.dbv"; // or directly the .dbvs

    let uart_status @ UART = 0;
    ```

## Validation

The Brief compiler's `HardwareValidator` uses the `.dbvs` schema to perform two critical checks at compile time:

1.  **Memory Bounds Checking**: It verifies that variables with physical addresses (e.g., `let x @ 0x...`) fall within a valid memory bank defined in the target specification (`.toml` file).
2.  **Alias Resolution**: It ensures that all aliases used in the Brief code are defined in the imported `.dbvs` schema.

This prevents memory overflow errors (like the one found on the KV260) and provides a single source of truth for the hardware layout.
