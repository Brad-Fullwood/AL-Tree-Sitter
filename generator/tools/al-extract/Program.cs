// al-extract: Extracts AL language data from Microsoft's CodeAnalysis DLLs.
//
// Outputs to ../../data/ (or --output <dir>):
//   builtin_functions.json  — global AL built-in functions (names + categories)
//   runtime_enums.json      — AL option types defined in the DLL
//   implicit_variables.json — trigger-context implicit variables (static, not in DLL)
//
// Usage: dotnet run [-- --output <dir>] [--dll <path>]
//        dotnet run -- --output ../../data
//
// Discovery strategy:
//   - Built-in functions: *StaticBuiltInMethodTypeSymbol nested classes in *ClassTypeSymbol
//     gives function name + category. Parameter info is NOT in metadata (requires runtime).
//     We enrich with parameter data from the existing JSON if present.
//   - Runtime enums: SystemOptionKinds+*Kind enums in the DLL give AL option values.
//   - Implicit variables: Not in the DLL; kept from static definitions below.

using System.Reflection;
using System.Text.Json;
using System.Text.Json.Serialization;

// ─── Argument parsing ──────────────────────────────────────────────────────────

string? outputDir = null;
string? explicitDll = null;

for (int i = 0; i < args.Length; i++)
{
    if (args[i] == "--output" && i + 1 < args.Length) outputDir = args[++i];
    else if (args[i] == "--dll" && i + 1 < args.Length) explicitDll = args[++i];
}

// Default output: relative to the assembly location.
// Assembly is at: tree-sitter-al/generator/tools/al-extract/bin/<config>/net8.0/
// Target data is: tree-sitter-al/data/
// That is 6 levels up from bin/<config>/net8.0/, then into "data".
if (outputDir == null)
{
    var exeDir = AppContext.BaseDirectory;
    outputDir = Path.GetFullPath(Path.Combine(exeDir, "..", "..", "..", "..", "..", "..", "data"));
}

outputDir = Path.GetFullPath(outputDir);
Console.Error.WriteLine($"Output directory: {outputDir}");
Directory.CreateDirectory(outputDir);

// ─── Find the AL extension DLL ─────────────────────────────────────────────────

string? dllPath = explicitDll;

if (dllPath == null)
{
    var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
    var searchRoots = new[]
    {
        Path.Combine(home, ".cursor", "extensions"),
        Path.Combine(home, ".vscode", "extensions"),
    };

    foreach (var root in searchRoots)
    {
        if (!Directory.Exists(root)) continue;
        // Find ms-dynamics-smb.al-* directories, pick highest version
        var alDirs = Directory.GetDirectories(root, "ms-dynamics-smb.al-*")
            .OrderByDescending(d => d) // lexicographic desc picks highest semver
            .ToList();
        foreach (var dir in alDirs)
        {
            // Try platform-specific bin first, then generic bin
            var platform = Environment.OSVersion.Platform switch
            {
                PlatformID.Unix => "linux",
                PlatformID.Win32NT => "win32",
                _ => "linux"
            };
            // Check if MacOS
            if (OperatingSystem.IsMacOS()) platform = "darwin";

            var candidates = new[]
            {
                Path.Combine(dir, "bin", platform, "Microsoft.Dynamics.Nav.CodeAnalysis.dll"),
                Path.Combine(dir, "bin", "Microsoft.Dynamics.Nav.CodeAnalysis.dll"),
            };
            foreach (var c in candidates)
            {
                if (File.Exists(c)) { dllPath = c; break; }
            }
            if (dllPath != null) break;
        }
        if (dllPath != null) break;
    }
}

if (dllPath == null)
{
    Console.Error.WriteLine("Error: Microsoft AL extension not found.");
    Console.Error.WriteLine("Searched:");
    Console.Error.WriteLine("  ~/.cursor/extensions/ms-dynamics-smb.al-*/bin/");
    Console.Error.WriteLine("  ~/.vscode/extensions/ms-dynamics-smb.al-*/bin/");
    Console.Error.WriteLine("Install the AL Language extension in VS Code or Cursor first.");
    Console.Error.WriteLine("Alternatively, pass --dll <path/to/Microsoft.Dynamics.Nav.CodeAnalysis.dll>");
    return 1;
}

Console.Error.WriteLine($"Using DLL: {dllPath}");
var binDir = Path.GetDirectoryName(dllPath)!;

// ─── Load the assembly with MetadataLoadContext ─────────────────────────────────

// Deduplicate by filename — MetadataLoadContext rejects duplicate assembly identities.
// Runtime assemblies must come first so mscorlib/System.* resolve correctly.
var seenNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
var resolverPaths = new List<string>();

void AddResolverPath(string p)
{
    var name = Path.GetFileName(p);
    if (seenNames.Add(name)) resolverPaths.Add(p);
}

var runtimeDir = Path.GetDirectoryName(typeof(object).Assembly.Location)!;
foreach (var f in Directory.GetFiles(runtimeDir, "*.dll")) AddResolverPath(f);
AddResolverPath(dllPath);
foreach (var f in Directory.GetFiles(binDir, "*.dll")) AddResolverPath(f);

MetadataLoadContext mlc;
Assembly codeAnalysis;
try
{
    var resolver = new PathAssemblyResolver(resolverPaths);
    mlc = new MetadataLoadContext(resolver);
    codeAnalysis = mlc.LoadFromAssemblyPath(dllPath);
}
catch (Exception ex)
{
    Console.Error.WriteLine($"Error loading assembly: {ex.Message}");
    return 1;
}

var allTypes = codeAnalysis.GetTypes();
Console.Error.WriteLine($"Loaded {allTypes.Length} types from {Path.GetFileName(dllPath)}");

// ─── Extract runtime enums ──────────────────────────────────────────────────────
//
// AL runtime option types live as SystemOptionKinds+*Kind enums.
// The naming convention is: {AlTypeName}Kind  e.g. CommitBehaviorKind -> CommitBehavior
// We emit only the ones that correspond to AL option keywords actually used at runtime.

Console.Error.WriteLine("\n--- Extracting runtime enums ---");

// These are the AL runtime option types we care about (not published in .app packages).
// Map: DLL enum name (without "Kind" suffix) -> AL type name as used in AL source
var runtimeEnumNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
{
    "WebServiceActionResultCode",
    "SecurityFilter",
    "DataScope",
    "ErrorBehavior",
    "TestPermissions",
    "TransactionModel",
    "CommitBehavior",
    "InherentPermissionsScope",
};

// Find all SystemOptionKinds+*Kind enums by searching for types whose full name matches
// the pattern Microsoft.Dynamics.Nav.CodeAnalysis.Symbols.SystemOptionKinds+*Kind
var runtimeEnumResults = new List<object>();

foreach (var type in allTypes)
{
    if (!type.IsEnum) continue;
    if (type.DeclaringType?.Name != "SystemOptionKinds") continue;

    // Strip "Kind" suffix to get the AL type name
    var kindName = type.Name; // e.g. "CommitBehaviorKind"
    if (!kindName.EndsWith("Kind")) continue;
    var alName = kindName[..^"Kind".Length]; // e.g. "CommitBehavior"

    if (!runtimeEnumNames.Contains(alName)) continue;

    var fields = type.GetFields(BindingFlags.Public | BindingFlags.Static);
    var values = fields.Select(f => f.Name).ToList();

    Console.Error.WriteLine($"  {alName}: [{string.Join(", ", values)}]");
    runtimeEnumResults.Add(new { name = alName, values });
}

// Report any that weren't found
foreach (var expected in runtimeEnumNames)
{
    if (!runtimeEnumResults.Any(r => ((dynamic)r).name == expected))
        Console.Error.WriteLine($"  WARNING: {expected} not found in DLL");
}

// ─── Extract built-in function names and categories ──────────────────────────────
//
// Global AL functions are defined as *StaticBuiltInMethodTypeSymbol nested classes
// inside *ClassTypeSymbol parent classes.
//
// Name extraction: strip "StaticBuiltInMethodTypeSymbol" suffix from the nested class name.
// Category: derived from the parent ClassTypeSymbol name.
//
// Parameter info is NOT available through MetadataLoadContext (requires runtime instantiation).
// Strategy: for each function we find in the DLL, check if the existing JSON has
// parameter data for it. If yes, use that. If no, emit a minimal entry.

Console.Error.WriteLine("\n--- Extracting built-in function names ---");

// Map parent class name suffix -> category string
static string ClassToCategory(string? parentName) => parentName switch
{
    null => "system",
    _ when parentName.StartsWith("Dialog") => "dialog",
    _ when parentName.StartsWith("Text") => "string",
    _ when parentName.StartsWith("System") => "system",
    _ when parentName.StartsWith("File") => "file",
    _ when parentName.StartsWith("Page") => "system",
    _ when parentName.StartsWith("EnumType") => "type",
    _ when parentName.StartsWith("Report") => "system",
    _ => "system"
};

// Gather all *StaticBuiltInMethodTypeSymbol types
var discoveredFunctions = new List<(string Name, string Category)>();

foreach (var type in allTypes)
{
    if (!type.Name.EndsWith("StaticBuiltInMethodTypeSymbol")) continue;
    if (type.Name.Contains("<")) continue; // skip compiler-generated

    var methodName = type.Name[..^"StaticBuiltInMethodTypeSymbol".Length];
    var category = ClassToCategory(type.DeclaringType?.Name);

    discoveredFunctions.Add((methodName, category));
    Console.Error.WriteLine($"  {category}/{methodName}");
}

Console.Error.WriteLine($"  Discovered {discoveredFunctions.Count} static built-in functions in DLL");

// Load existing JSON to use as parameter data source
var existingFunctions = new List<JsonElement>();
var existingFunctionsPath = Path.Combine(outputDir, "builtin_functions.json");
if (File.Exists(existingFunctionsPath))
{
    try
    {
        var json = File.ReadAllText(existingFunctionsPath);
        var doc = JsonDocument.Parse(json);
        foreach (var el in doc.RootElement.EnumerateArray())
            existingFunctions.Add(el.Clone());
        Console.Error.WriteLine($"  Loaded {existingFunctions.Count} entries from existing JSON for enrichment");
    }
    catch (Exception ex)
    {
        Console.Error.WriteLine($"  WARNING: Could not load existing builtin_functions.json: {ex.Message}");
    }
}

// Build a lookup from existing data: name -> JsonElement
var existingByName = existingFunctions
    .Where(e => e.TryGetProperty("name", out _))
    .ToDictionary(
        e => e.GetProperty("name").GetString()!,
        e => e,
        StringComparer.OrdinalIgnoreCase);

// For built-in functions output: merge DLL-discovered with existing data.
// DLL is authoritative for: existence of function, category assignment.
// Existing JSON is authoritative for: parameters, signatures, descriptions.
//
// We include ALL existing functions in the output (they are authoritative for
// functions that aren't in DLL static types too, such as string/math functions
// that may be implemented differently in the type system).

// First: emit all DLL-discovered functions, enriched from existing where available
var builtinOutput = new List<object>();
var emittedNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

foreach (var (name, category) in discoveredFunctions.OrderBy(f => f.Name))
{
    if (emittedNames.Contains(name)) continue;
    emittedNames.Add(name);

    if (existingByName.TryGetValue(name, out var existing))
    {
        // Use existing data but override category from DLL
        var entry = RebuildWithCategory(existing, category);
        builtinOutput.Add(entry);
    }
    else
    {
        // New function found in DLL with no existing data
        Console.Error.WriteLine($"  NEW function from DLL (no existing data): {name}");
        builtinOutput.Add(new
        {
            name,
            signature = $"{name}(...)",
            parameters = Array.Empty<object>(),
            return_type = (string?)null,
            description = $"Built-in AL function.",
            category
        });
    }
}

// Second: include existing functions not found in DLL (they are still valid AL functions)
foreach (var existing in existingFunctions)
{
    if (!existing.TryGetProperty("name", out var nameProp)) continue;
    var name = nameProp.GetString()!;
    if (emittedNames.Contains(name)) continue;
    emittedNames.Add(name);
    builtinOutput.Add(existing);
}

Console.Error.WriteLine($"  Total built-in functions in output: {builtinOutput.Count}");

// ─── Implicit variables ─────────────────────────────────────────────────────────
//
// These are not discoverable from the DLL metadata — they are language-spec defined
// implicit variables available in trigger bodies. We keep the static definitions.

Console.Error.WriteLine("\n--- Implicit variables (static, not in DLL) ---");
Console.Error.WriteLine("  Keeping existing implicit_variables.json (not extractable from DLL)");

// ─── Write output files ─────────────────────────────────────────────────────────

var jsonOptions = new JsonSerializerOptions
{
    WriteIndented = true,
    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
};

// runtime_enums.json
var enumsPath = Path.Combine(outputDir, "runtime_enums.json");
var enumsJson = JsonSerializer.Serialize(runtimeEnumResults, jsonOptions);
await File.WriteAllTextAsync(enumsPath, enumsJson + "\n");
Console.Error.WriteLine($"\nWrote {enumsPath}");

// builtin_functions.json
// We write it as raw JSON to preserve the existing structure for enriched entries
var builtinsPath = Path.Combine(outputDir, "builtin_functions.json");
await WriteBuiltinFunctionsAsync(builtinsPath, builtinOutput);
Console.Error.WriteLine($"Wrote {builtinsPath}");

Console.Error.WriteLine("\nExtraction complete.");
return 0;

// ─── Helper functions ───────────────────────────────────────────────────────────

// Rebuilds a JsonElement with an updated category field
static object RebuildWithCategory(JsonElement existing, string newCategory)
{
    // We serialize the existing element to a dict and update the category
    using var ms = new System.IO.MemoryStream();
    using var writer = new Utf8JsonWriter(ms);
    writer.WriteStartObject();
    foreach (var prop in existing.EnumerateObject())
    {
        if (prop.Name == "category")
        {
            writer.WriteString("category", newCategory);
        }
        else
        {
            prop.WriteTo(writer);
        }
    }
    writer.WriteEndObject();
    writer.Flush();
    return JsonDocument.Parse(ms.ToArray()).RootElement.Clone();
}

// Writes the builtin functions list, handling both anonymous objects and JsonElements
static async Task WriteBuiltinFunctionsAsync(string path, List<object> items)
{
    using var ms = new System.IO.MemoryStream();
    var opts = new JsonWriterOptions { Indented = true };
    using var writer = new Utf8JsonWriter(ms, opts);

    writer.WriteStartArray();
    foreach (var item in items)
    {
        if (item is JsonElement je)
        {
            je.WriteTo(writer);
        }
        else
        {
            JsonSerializer.Serialize(writer, item, item.GetType());
        }
    }
    writer.WriteEndArray();
    await writer.FlushAsync();

    // Add trailing newline
    var bytes = ms.ToArray();
    using var fs = File.Create(path);
    await fs.WriteAsync(bytes);
    await fs.WriteAsync("\n"u8.ToArray());
}
