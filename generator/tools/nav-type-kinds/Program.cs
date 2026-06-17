// Extracts the Microsoft.Dynamics.Nav.CodeAnalysis.NavTypeKind enum (name -> id)
// from the AL toolchain DLL and prints it as JSON. See README.md.
//
// Usage: TCDIR=<toolchain net8.0/any dir> dotnet run -c Release > nav_type_kinds.json

using System.Reflection;
using System.Text.Json;

var tcdir = Environment.GetEnvironmentVariable("TCDIR")
    ?? throw new InvalidOperationException("Set TCDIR to the AL toolchain dir containing Microsoft.Dynamics.Nav.CodeAnalysis.dll");

AppDomain.CurrentDomain.AssemblyResolve += (_, a) =>
{
    var p = Path.Combine(tcdir, new AssemblyName(a.Name).Name + ".dll");
    return File.Exists(p) ? Assembly.LoadFrom(p) : null;
};

var asm = Assembly.LoadFrom(Path.Combine(tcdir, "Microsoft.Dynamics.Nav.CodeAnalysis.dll"));
var t = asm.GetType("Microsoft.Dynamics.Nav.CodeAnalysis.NavTypeKind")
    ?? throw new InvalidOperationException("NavTypeKind type not found");

var map = new SortedDictionary<string, int>(StringComparer.Ordinal);
foreach (var v in Enum.GetValues(t))
{
    var name = Enum.GetName(t, v)!;
    // Skip the internal flag bits (leading underscore); keep real AL type kinds.
    if (name.StartsWith('_')) continue;
    map[name] = Convert.ToInt32(v);
}

Console.WriteLine(JsonSerializer.Serialize(map, new JsonSerializerOptions { WriteIndented = true }));
