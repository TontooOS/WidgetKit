# Tontoo Name

Description

## Made for TontooOS

Explore more at https://github.com/TontooOS/Libs

## Adding to Your Project

Add to your `Cargo.toml`:

```toml
[dependencies]
sdk = { path = "/Library/System/sdk", features = ["Name"] }
```

Then at the crate root:

```rust
sdk::preinclude!();
use Name::{{ /* ... */ }};
```

## License

TCL v27.0