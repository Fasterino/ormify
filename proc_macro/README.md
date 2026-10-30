# ormify_proc_macro

Procedural macros used by [`ormify`](https://crates.io/crates/ormify). Most application crates should enable the `proc_macro` feature on `ormify` and use its re-exports rather than depend on `ormify_proc_macro` directly.

```toml
[dependencies]
ormify = { version = "1", features = ["full"] }
```

## Macros

### `#[derive(Entity)]`

Generates the `ormify::Entity` implementation for a struct with named fields. The struct must declare at least one primary key with `#[db(pk)]`. Generated items include a builder, typed condition/order-by builders, schema metadata, entry conversion, and foreign-key join metadata.

Struct attributes:

- `#[db(group = expression)]` sets the optional group/schema. The expression is emitted as Rust code, so a constant such as `const GROUP: &str = "app";` may be used.
- `#[db(rename = expression)]` changes the entity/table name.

Field attributes:

- `#[db(rename = expression)]` changes the column name.
- `#[db(pk)]` marks a primary-key field.
- `#[db(unique)]` or `#[db(unique: name)]` declares a unique constraint; matching names group fields into a composite constraint.
- `#[db(index)]` or `#[db(index: name)]` declares an index; matching names group fields into a composite index.
- `#[db(fk -> referenced_key in Entity)]` declares a foreign key.
- `#[db(fk: name -> referenced_key in Entity)]` gives the foreign key a name; matching names group fields into a composite foreign key.
- `#[db(update = path)]` sets a function/path used to refresh the field when `Observer::update()` is built.

### `#[derive(FieldType)]`

Generates the core `FieldType` implementation for supported enum forms:

- `#[ft(VarChar(N))]`: stores enum variant names as strings; variants must be unit variants.
- `#[ft(i16)]`, `#[ft(i32)]`, or `#[ft(i64)]`: stores enum discriminants as the selected signed integer type; variants must be compatible unit variants.
- `#[ft(Vec<u8>)]`: encodes and decodes the whole type as CBOR; derive `Serialize` and `Deserialize` as well.

### `c!(...)`

Parses a field expression and emits an `ormify::Condition`. Syntax includes equality and ordering operators, list membership, inclusive ranges, and string-like matching:

```rust
c!(user.id == id)
c!(user.age in [18, 21, 65])
c!(user.age in [18..=65])
c!(user.nickname =? "%ada%")
```

Conditions can be combined with `&` and `|` in Rust. `=?` maps to `LIKE`; wildcard characters are interpreted by PostgreSQL.

## Generated crate path

The macro currently expands references to the crate name `ormify`. Consumers should make the crate available under that name (normally through the `ormify` dependency). `shared_subcrate` is a legacy feature and is not currently part of the `ormify` feature table.

## Compatibility

The proc-macro crate follows the version of the workspace's ORM API. Prefer enabling macros through the matching `ormify` release so generated code and core traits stay compatible.
