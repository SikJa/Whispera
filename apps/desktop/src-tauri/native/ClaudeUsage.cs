// Receives Claude Code's documented statusLine JSON. Keeps only quota fields.
// No network, authentication files, transcripts, prompts, paths or session IDs.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;
using System.Web.Script.Serialization;

internal static class ClaudeUsage {
    static readonly JavaScriptSerializer Json = new JavaScriptSerializer { MaxJsonLength = 262144 };
    static Dictionary<string, object> Object(Dictionary<string, object> source, string key) {
        object value;
        return source != null && source.TryGetValue(key, out value) ? value as Dictionary<string, object> : null;
    }
    static double? Number(Dictionary<string, object> source, string key) {
        object value;
        if (source == null || !source.TryGetValue(key, out value) ||
            !(value is int || value is long || value is double || value is decimal)) return null;
        double number = Convert.ToDouble(value, CultureInfo.InvariantCulture);
        return Double.IsNaN(number) || Double.IsInfinity(number) || number < 0 ? (double?)null : number;
    }
    static Dictionary<string, object> Window(Dictionary<string, object> source, bool money) {
        var value = new Dictionary<string, object>();
        double? used = Number(source, "used_percentage");
        if (!used.HasValue) return null;
        value["used_percentage"] = used.Value;
        double? reset = Number(source, "resets_at");
        if (reset.HasValue && reset.Value > 0 && reset.Value < 4102444800) value["resets_at"] = (long)reset.Value;
        if (money) {
            foreach (string key in new[] { "used_usd", "limit_usd" }) {
                double? amount = Number(source, key);
                if (amount.HasValue) value[key] = amount.Value;
            }
        }
        return value;
    }
    static int Main(string[] args) {
        string temporary = null;
        Console.InputEncoding = new UTF8Encoding(false);
        Console.OutputEncoding = new UTF8Encoding(false);
        try {
            if (args.Length != 1 || !Path.IsPathRooted(args[0])) return 1;
            var text = new StringBuilder();
            var buffer = new char[4096];
            int length;
            while ((length = Console.In.Read(buffer, 0, buffer.Length)) > 0) {
                if (text.Length + length > 262144) return 1;
                text.Append(buffer, 0, length);
            }
            var input = Json.Deserialize<Dictionary<string, object>>(text.ToString());
            var limits = Object(input, "rate_limits");
            var clean = new Dictionary<string, object>();
            var display = new List<string>();
            foreach (string key in new[] { "five_hour", "seven_day", "spend_limit" }) {
                var window = Window(Object(limits, key), key == "spend_limit");
                if (window == null) continue;
                clean[key] = window;
                if (key != "spend_limit") display.Add((key == "five_hour" ? "5h: " : "7d: ") +
                    Math.Max(0, 100 - (double)window["used_percentage"]).ToString("0.#", CultureInfo.InvariantCulture) + "% libre");
            }
            var snapshot = new { schema = 1, observedAt = (long)(DateTime.UtcNow - new DateTime(1970, 1, 1)).TotalSeconds, rate_limits = clean };
            string directory = Path.GetDirectoryName(args[0]);
            Directory.CreateDirectory(directory);
            temporary = Path.Combine(directory, ".claude-" + Guid.NewGuid().ToString("N") + ".tmp");
            File.WriteAllText(temporary, Json.Serialize(snapshot), new UTF8Encoding(false));
            if (File.Exists(args[0])) File.Replace(temporary, args[0], null);
            else {
                try { File.Move(temporary, args[0]); }
                catch (IOException) { File.Replace(temporary, args[0], null); }
            }
            Console.WriteLine(display.Count > 0 ? "Claude | " + String.Join(" | ", display) : "Claude | esperando cuotas");
            return 0;
        } catch {
            Console.WriteLine("Claude | cuotas no disponibles");
            return 1;
        } finally {
            if (temporary != null) { try { File.Delete(temporary); } catch {} }
        }
    }
}
