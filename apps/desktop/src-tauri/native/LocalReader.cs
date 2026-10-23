// Local-only speech bridge. Input is JSON on stdin, never command-line code.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Speech.Synthesis;
using System.Web.Script.Serialization;

internal static class LocalReader {
    sealed class ReaderInputException : Exception { public ReaderInputException(string message) : base(message) {} }
    const int MaximumTextLength = 50000;
    static readonly JavaScriptSerializer Json = new JavaScriptSerializer { MaxJsonLength = 400000 };
    static SpeechSynthesizer Synth;
    static readonly List<object> Voices = new List<object>();
    static readonly HashSet<string> VoiceNames = new HashSet<string>(StringComparer.Ordinal);

    static object Snapshot() {
        return new { ok = true, supported = true, phase = Synth.State == SynthesizerState.Speaking ? "speaking" : Synth.State == SynthesizerState.Paused ? "paused" : "idle", voices = Voices };
    }
    static string RequiredString(Dictionary<string, object> request, string key) {
        object value;
        if (!request.TryGetValue(key, out value) || !(value is string)) throw new ReaderInputException("Solicitud de lectura inválida.");
        return (string)value;
    }
    static object Handle(Dictionary<string, object> request) {
        string action = RequiredString(request, "action");
        switch (action) {
            case "status": break;
            case "speak": case "render-wave": {
                string text = RequiredString(request, "text");
                string voice = RequiredString(request, "voice");
                object rawRate;
                int rate;
                if (String.IsNullOrWhiteSpace(text) || text.Length > MaximumTextLength || text.IndexOf('\0') >= 0) throw new ReaderInputException("Usá entre 1 y 50.000 caracteres de texto.");
                if (!request.TryGetValue("rate", out rawRate) || !Int32.TryParse(Convert.ToString(rawRate, CultureInfo.InvariantCulture), out rate) || rate < -10 || rate > 10) throw new ReaderInputException("Velocidad no válida.");
                if (!VoiceNames.Contains(voice)) throw new ReaderInputException("La voz elegida no está instalada.");
                if (action == "render-wave") {
                    string path = RequiredString(request, "path");
                    if (!Path.IsPathRooted(path)) throw new ReaderInputException("Ruta de audio no válida.");
                    Synth.SelectVoice(voice);
                    Synth.Rate = rate;
                    // A separate one-shot process renders silently to a private staging file.
                    Synth.SetOutputToWaveFile(path);
                    try { Synth.Speak(text); } finally { Synth.SetOutputToNull(); }
                    break;
                }
                Synth.SpeakAsyncCancelAll();
                if (Synth.State == SynthesizerState.Paused) Synth.Resume();
                Synth.SelectVoice(voice);
                Synth.Rate = rate;
                // SpeakAsync(string) reads literal text, never SSML or executable input.
                Synth.SpeakAsync(text);
                break;
            }
            case "pause": if (Synth.State == SynthesizerState.Speaking) Synth.Pause(); break;
            case "resume": if (Synth.State == SynthesizerState.Paused) Synth.Resume(); break;
            case "stop":
                Synth.SpeakAsyncCancelAll();
                if (Synth.State == SynthesizerState.Paused) Synth.Resume();
                break;
            default: throw new ReaderInputException("Acción de lectura no admitida.");
        }
        return Snapshot();
    }
    [STAThread]
    static int Main(string[] args) {
        Console.InputEncoding = new System.Text.UTF8Encoding(false);
        Console.OutputEncoding = new System.Text.UTF8Encoding(false);
        try {
            using (Synth = new SpeechSynthesizer()) {
                foreach (InstalledVoice installed in Synth.GetInstalledVoices()) {
                    if (!installed.Enabled) continue;
                    VoiceInfo voice = installed.VoiceInfo;
                    VoiceNames.Add(voice.Name);
                    Voices.Add(new { id = voice.Name, name = voice.Name, language = voice.Culture.Name, code = voice.Culture.ThreeLetterISOLanguageName });
                }
                string line;
                while ((line = Console.ReadLine()) != null) {
                    try {
                        if (line.Length > 350000) throw new ReaderInputException("Solicitud demasiado grande.");
                        var request = Json.Deserialize<Dictionary<string, object>>(line);
                        bool render = args.Length == 1 && args[0] == "--render-wave";
                        if ((RequiredString(request, "action") == "render-wave") != render) throw new ReaderInputException("Acción de lectura no admitida.");
                        Console.WriteLine(Json.Serialize(Handle(request)));
                        if (render) return 0;
                    } catch (Exception error) {
                        // No input text or raw engine exception is written to logs/output.
                        string message = error is ReaderInputException ? error.Message : "Windows no pudo completar la lectura. Probá otra voz.";
                        Console.WriteLine(Json.Serialize(new { ok = false, error = message }));
                        if (args.Length != 0) return 1;
                    }
                    Console.Out.Flush();
                }
                Synth.SpeakAsyncCancelAll();
            }
            return 0;
        } catch {
            Console.WriteLine(Json.Serialize(new { ok = false, error = "El motor de voz de Windows no está disponible." }));
            return 1;
        }
    }
}
