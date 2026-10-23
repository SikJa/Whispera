fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") { build_windows_reader(); build_windows_usage(); }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_macos_bridge();
    }
    tauri_build::build()
}

fn build_windows_usage() {
    use std::{env,path::PathBuf,process::Command};
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source=root.join("native").join("ClaudeUsage.cs");
    println!("cargo:rerun-if-changed={}",source.display());
    let out=root.join("target").join("reader");std::fs::create_dir_all(&out).unwrap();
    let compiler=PathBuf::from(env::var_os("WINDIR").unwrap()).join("Microsoft.NET").join("Framework64").join("v4.0.30319").join("csc.exe");
    let result=Command::new(compiler).args(["/nologo","/target:exe","/optimize+","/reference:System.Web.Extensions.dll"])
        .arg(format!("/out:{}",out.join("WhisperaClaudeUsage.exe").display())).arg(source).output().expect("Windows .NET Framework compiler required");
    assert!(result.status.success(),"Claude usage bridge did not compile: {}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
}

fn build_macos_bridge() {
    use std::{env, path::PathBuf, process::Command};
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("Unsupported macOS architecture: {other}"),
    };
    let sdk = Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"])
        .output().expect("Xcode command line tools are required");
    assert!(sdk.status.success(), "Could not locate the macOS SDK");
    let sdk = String::from_utf8(sdk.stdout).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut sources: Vec<_> = std::fs::read_dir("src/macos").unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("swift"))
        .collect();
    sources.sort();
    println!("cargo:rerun-if-changed=src/macos");
    let status = Command::new("xcrun").arg("swiftc")
        .args(["-swift-version", "5", "-parse-as-library", "-emit-library", "-static", "-O",
            "-module-name", "WhisperaNative", "-target", &format!("{arch}-apple-macosx13.0"),
            "-sdk", sdk.trim()])
        .args(sources).arg("-o").arg(out.join("libwhispera_native.a"))
        .status().expect("Could not invoke the Swift compiler");
    assert!(status.success(), "Whispera macOS native bridge did not compile");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=whispera_native");
    let swift = Command::new("xcrun").args(["--find", "swiftc"]).output().unwrap();
    let compiler = PathBuf::from(String::from_utf8(swift.stdout).unwrap().trim());
    let toolchain = compiler.parent().unwrap().parent().unwrap();
    println!("cargo:rustc-link-search=native={}", toolchain.join("lib/swift/macosx").display());
    println!("cargo:rustc-link-search=native={}/usr/lib/swift", sdk.trim());
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    for framework in ["AppKit", "Foundation", "ApplicationServices", "CoreGraphics", "AVFoundation",
        "ScreenCaptureKit", "CoreAudio", "AudioToolbox", "CoreMedia", "CoreVideo", "ImageIO",
        "UniformTypeIdentifiers", "QuartzCore"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

fn build_windows_reader() {
    use std::{env, path::PathBuf, process::Command};
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("native").join("LocalReader.cs");
    println!("cargo:rerun-if-changed={}", source.display());
    let out = root.join("target").join("reader");
    std::fs::create_dir_all(&out).expect("Reader output directory");
    let framework = PathBuf::from(env::var_os("WINDIR").expect("Windows directory"))
        .join("Microsoft.NET").join("Framework64").join("v4.0.30319");
    let compiler = framework.join("csc.exe");
    let result = Command::new(compiler).args(["/nologo", "/target:exe", "/optimize+", "/reference:System.Web.Extensions.dll"])
        .arg(format!("/reference:{}", framework.join("WPF").join("System.Speech.dll").display()))
        .arg(format!("/out:{}", out.join("WhisperaReader.exe").display())).arg(source)
        .output().expect("The Windows .NET Framework C# compiler is required for local reading");
    assert!(result.status.success(), "Reader compilation failed: {}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}
