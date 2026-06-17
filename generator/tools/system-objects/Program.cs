// Extracts the platform "system object" permission table (display name -> id)
// from the AL toolchain DLL and prints it as JSON. See README.md.
//
// The ids are `Microsoft.Dynamics.Nav.CodeAnalysis...SystemObjects` int constants
// (e.g. ToolsObjectDesigner = 5210); the AL display names ("Tools, Object
// Designer") are the embedded `SystemObjectsResources` strings keyed
// `{ConstName}SystemObjectCaption`. We join them by ConstName.
//
// Usage: TCDIR=<toolchain net8.0/any dir> dotnet run -c Release > system_objects.json

using System.Collections;
using System.Reflection;
using System.Resources;
using System.Text.Json;

var tcdir = Environment.GetEnvironmentVariable("TCDIR")
    ?? throw new InvalidOperationException("Set TCDIR to the AL toolchain dir containing Microsoft.Dynamics.Nav.CodeAnalysis.dll");

AppDomain.CurrentDomain.AssemblyResolve += (_, a) =>
{
    var p = Path.Combine(tcdir, new AssemblyName(a.Name).Name + ".dll");
    return File.Exists(p) ? Assembly.LoadFrom(p) : null;
};

var asm = Assembly.LoadFrom(Path.Combine(tcdir, "Microsoft.Dynamics.Nav.CodeAnalysis.dll"));

// SystemObjects const int fields: ConstName -> id.
var t = asm.GetType("Microsoft.Dynamics.Nav.CodeAnalysis.Binder.Semantics.SystemObjects")
    ?? asm.GetTypes().First(x => x.Name == "SystemObjects");
var consts = new Dictionary<string, int>();
foreach (var f in t.GetFields(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static))
{
    if (f.IsLiteral && f.FieldType == typeof(int))
        consts[f.Name] = (int)f.GetRawConstantValue()!;
}

// SystemObjectsResources: {ConstName}SystemObjectCaption -> display name.
var res = new Dictionary<string, string>();
var resName = asm.GetManifestResourceNames().First(n => n.Contains("SystemObjectsResources"));
using (var stream = asm.GetManifestResourceStream(resName))
using (var reader = new ResourceReader(stream!))
{
    foreach (DictionaryEntry e in reader)
        if (e.Value is string s)
            res[(string)e.Key] = s;
}

// Join by ConstName -> { display name: id }.
var map = new SortedDictionary<string, int>(StringComparer.Ordinal);
foreach (var (cn, id) in consts)
    if (res.TryGetValue(cn + "SystemObjectCaption", out var display))
        map[display] = id;

Console.WriteLine(JsonSerializer.Serialize(map, new JsonSerializerOptions { WriteIndented = true }));
