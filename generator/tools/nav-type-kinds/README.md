# nav-type-kinds generator

Generates `tree-sitter-al/data/nav_type_kinds.json` — the `NavTypeKind` name → id
table used by the native `.app` emitter's method-id hashing
(`crate::emit::method_id`).

These ids are Microsoft `Microsoft.Dynamics.Nav.CodeAnalysis.NavTypeKind` enum
values. Unlike the rest of the AL language data (sourced from the AL VS Code
extension's `.tmlanguage` by `al-gen`), these values live only in the compiled
`Microsoft.Dynamics.Nav.CodeAnalysis.dll`, so they are extracted by reflection
here. The output is committed as a data file like every other generated table;
**do not hand-edit it — regenerate.**

## Regenerate

```sh
# Point TCDIR at the AL toolchain dir that contains
# Microsoft.Dynamics.Nav.CodeAnalysis.dll (next to alc.dll).
TCDIR="$HOME/.local/bin/.store/microsoft.dynamics.businesscentral.development.tools/<version>/.../tools/net8.0/any" \
DOTNET_ROLL_FORWARD=Major dotnet run -c Release \
  > ../../../data/nav_type_kinds.json
cp ../../../data/nav_type_kinds.json ../../../../grammars/al/data/nav_type_kinds.json
```

Re-run after a toolchain (BC platform) upgrade so the ids track the runtime the
emitter targets.
