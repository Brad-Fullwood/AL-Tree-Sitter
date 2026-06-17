# system-objects generator

Generates `tree-sitter-al/data/system_objects.json` — the platform **system
permission object** display-name → id table used by the native `.app` emitter to
resolve `system "…" = …` permissions (e.g. `system "Tools, Object Designer"` →
id `5210`).

These ids are platform built-ins, **not** present in any `.alpackages` symbol
file. They live only in the compiled `Microsoft.Dynamics.Nav.CodeAnalysis.dll`:
the ids are `SystemObjects` int constants (`ToolsObjectDesigner = 5210`, …) and
the AL display names are the embedded `SystemObjectsResources` strings keyed
`{ConstName}SystemObjectCaption`. This tool reads both by reflection and joins
them by `ConstName`. The output is committed as a data file like every other
generated table; **do not hand-edit it — regenerate.**

## Regenerate

```sh
# Point TCDIR at the AL toolchain dir that contains
# Microsoft.Dynamics.Nav.CodeAnalysis.dll (next to alc.dll).
TCDIR="$HOME/.local/bin/.store/microsoft.dynamics.businesscentral.development.tools/<version>/.../tools/net8.0/any" \
DOTNET_ROLL_FORWARD=Major dotnet run -c Release \
  > ../../../data/system_objects.json
```

Re-run after a toolchain (BC platform) upgrade so the ids track the runtime the
emitter targets.
