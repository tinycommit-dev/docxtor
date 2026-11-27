// Validates .docx files against the Open XML rules with Microsoft's Open XML SDK validator.
//
//   dotnet run -c Release --project tools/ooxml-validate -- a.docx b.docx
//
// Prints one JSON object per file (JSON lines) with the number of errors and each error's id, part, path and
// description. A file the SDK cannot open at all is one error with the id "Open". Used by scripts/validate.mjs.
using System.Text.Json;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

var options = new JsonSerializerOptions { WriteIndented = false };

foreach (var path in args)
{
    var errors = new List<Dictionary<string, string>>();
    try
    {
        // Open a copy in memory, read-only: the validator must never change the file.
        using var stream = new MemoryStream(File.ReadAllBytes(path));
        using var doc = WordprocessingDocument.Open(stream, false);
        var validator = new OpenXmlValidator(FileFormatVersions.Office2019);
        foreach (var e in validator.Validate(doc))
        {
            errors.Add(new Dictionary<string, string>
            {
                ["id"] = e.Id ?? "",
                ["type"] = e.ErrorType.ToString(),
                ["part"] = e.Part?.Uri?.ToString() ?? "",
                ["path"] = e.Path?.XPath ?? "",
                ["description"] = e.Description ?? "",
            });
        }
    }
    catch (Exception ex)
    {
        errors.Add(new Dictionary<string, string>
        {
            ["id"] = "Open",
            ["type"] = "Open",
            ["part"] = "",
            ["path"] = "",
            ["description"] = ex.GetType().Name + ": " + ex.Message.Split('\n')[0],
        });
    }
    Console.WriteLine(JsonSerializer.Serialize(new { file = path, errorCount = errors.Count, errors }, options));
}
