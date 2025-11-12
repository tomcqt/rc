use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
  let args: Vec<String> = env::args().collect();

  if args.len() < 2 {
    eprintln!("Usage: rc <file.riff> [-o <output_path>]");
    std::process::exit(2);
  }

  println!("[_] rc {}\n", env!("CARGO_PKG_VERSION"));

  println!("[i] Input: {}", args[1]);

  let input_path = &args[1];
  let mut output_path: Option<PathBuf> = None;

  // Parse -o flag if present
  if args.len() > 2 && args[2] == "-o" && args.len() > 3 {
    output_path = Some(PathBuf::from(&args[3]));
  }

  let code = fs::read_to_string(input_path).expect("[x] failed to read input file");

  // Validate syntax before compilation
  if let Err(e) = validate_rf_syntax(&code, input_path) {
    eprintln!("{}", e);
    std::process::exit(1);
  }

  // output paths
  let base = Path::new(input_path)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("out");
  
  // Intermediate Rust file always goes to temp directory
  let temp_dir = env::temp_dir().join("rc_build");
  if !temp_dir.exists() {
    std::fs::create_dir_all(&temp_dir).expect("[x] failed to create temp directory");
  }

  // Final executable path
  let exe_path = if let Some(custom_path) = output_path {
    custom_path
  } else {
    PathBuf::from("./dist").join(base)
  };

  println!("[i] Output: {}", exe_path.to_string_lossy());

  // Create parent directory if it doesn't exist
  if let Some(parent) = exe_path.parent() {
    if !parent.as_os_str().is_empty() && !parent.exists() {
      std::fs::create_dir_all(parent).expect("[x] failed to create output directory");
    }
  }

  let now = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .expect("Time went backwards")
    .as_secs();

  let file_name = format!("build-{}-{}", base, now);

  let rs_path = temp_dir.join(format!("{}.rs", file_name));

  let generated = generate_rust_program(&code);
  fs::write(&rs_path, generated).expect("[x] failed to write generated rust file");

  print!("[i] Compiling... ");
  std::io::Write::flush(&mut std::io::stdout()).unwrap();

  // Call rustc to compile the generated file into an executable
  let status = Command::new("rustc")
    .arg(rs_path.to_string_lossy().to_string())
    .arg("-O")
    .arg("-o")
    .arg(exe_path.to_string_lossy().to_string())
    .status()
    .expect("[x] failed to spawn rustc - is rustc installed?");

  println!("done.");

  if !status.success() {
    eprintln!("[x] rustc failed to compile generated program");
    std::process::exit(3);
  }

  println!("[i] Generated executable at {}", exe_path.to_string_lossy());
}

/// Validate RF syntax before compilation
fn validate_rf_syntax(code: &str, filename: &str) -> Result<(), String> {
  let mut line_num = 1;
  let mut col_num = 1;
  let mut i = 0;
  let bytes = code.as_bytes();
  let mut brace_stack: Vec<(usize, usize)> = Vec::new(); // (line, col) of opening braces
  let mut bracket_stack: Vec<(usize, usize)> = Vec::new(); // (line, col) of opening brackets

  while i < bytes.len() {
    let c = bytes[i] as char;

    // Track line and column numbers
    if c == '\n' {
      line_num += 1;
      col_num = 1;
      i += 1;
      continue;
    }
    col_num += 1;

    // Skip whitespace and comments
    if c.is_whitespace() {
      i += 1;
      continue;
    }
    if c == '@' {
      // Skip until end of line
      while i < bytes.len() && (bytes[i] as char) != '\n' {
        i += 1;
      }
      continue;
    }

    // Check for brace matching
    if c == '{' {
      brace_stack.push((line_num, col_num));
      i += 1;
      continue;
    }
    if c == '}' {
      if brace_stack.is_empty() {
        return Err(format!(
          "{}:{}:{}: error: unmatched '}}'\n  Unexpected closing brace",
          filename, line_num, col_num
        ));
      }
      brace_stack.pop();
      i += 1;
      continue;
    }

    // Check for bracket matching (in lists and macros)
    if c == '[' {
      bracket_stack.push((line_num, col_num));
      i += 1;
      continue;
    }
    if c == ']' {
      if bracket_stack.is_empty() {
        return Err(format!(
          "{}:{}:{}: error: unmatched ']'\n  Unexpected closing bracket",
          filename, line_num, col_num
        ));
      }
      bracket_stack.pop();
      i += 1;
      continue;
    }

    i += 1;
  }

  // Check for unclosed braces
  if let Some((line, _)) = brace_stack.pop() {
    return Err(format!(
      "{}:{}:{}: error: unmatched '{{'\n  Opening brace at line {} never closed",
      filename, line_num, col_num, line
    ));
  }

  // Check for unclosed brackets
  if let Some((line, _)) = bracket_stack.pop() {
    return Err(format!(
      "{}:{}:{}: error: unmatched '['\n  Opening bracket at line {} never closed",
      filename, line_num, col_num, line
    ));
  }

  Ok(())
}

/// Produce a standalone Rust program string that embeds a small RF interpreter and the code.
fn generate_rust_program(code: &str) -> String {
  // escape backslashes, quotes, CR, and newlines so the generated Rust string literal stays valid
  let escaped_code: String = code
    .replace("\\", "\\\\")
    .replace('"', "\\\"")
    .replace("\r", "")
    .replace("\n", "\\n");
  let template: &str = include_str!("../template/main.rs");
  template.replace("__RF_CODE_ESCAPED__", &escaped_code)
}