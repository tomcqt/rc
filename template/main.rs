use std::collections::HashMap;

fn main() {
  // embedded Riff code is replaced at __RF_CODE_ESCAPED__
  let code = "__RF_CODE_ESCAPED__";
  if let Err(e) = run(code) {
    eprintln!("\nRuntime error: {}\n", e);
    std::process::exit(1);
  }
}

#[derive(Debug, Clone)]
enum Val {
    Int(i64),
    Str(String),
    List(Vec<Val>),
}

impl Val {
    fn as_i64(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Str(s) => s.parse().unwrap_or(0),
            Val::List(v) => v.iter().map(|x| x.as_i64()).sum(),
        }
    }
    fn as_string(&self) -> String {
        match self {
            Val::Int(i) => i.to_string(),
            Val::Str(s) => s.clone(),
            Val::List(v) => {
                let parts: Vec<String> = v.iter().map(|x| x.as_string()).collect();
                format!("[{}]", parts.join(","))
            }
        }
    }
}

fn run(code: &str) -> Result<(), String> {
    let mut vars: HashMap<String, Val> = HashMap::new();
    run_block_simple_loop(code, &mut vars)
}

fn run_block_simple_loop(code: &str, vars: &mut HashMap<String, Val>) -> Result<(), String> {
    let mut i = 0usize;
    let bytes = code.as_bytes();
    while i < bytes.len() {
        // skip whitespace
        while i < bytes.len() && (bytes[i] as char).is_whitespace() { i += 1; }
        if i >= bytes.len() { break; }
        let c = bytes[i] as char;
        if c == '@' { // comment
            while i < bytes.len() && (bytes[i] as char) != '\n' { i += 1; }
            continue;
        } else if c == '?' || (c == '!' && i + 1 < bytes.len() && ((bytes[i+1] as char)=='?' || (bytes[i+1] as char)=='!')) {
            // if / else-if / else chain
            handle_if_chain(code, &mut i, vars)?;
            continue;
        } else if c == '"' {
            // string literal then expect >
            let (lit, ni) = extract_string(code, i)?;
            i = ni;
            skip_ws_bytes(code.as_bytes(), &mut i);
            if i < bytes.len() && (bytes[i] as char) == '>' { i += 1; skip_ws_bytes(code.as_bytes(), &mut i); if i < bytes.len() && (bytes[i] as char) == '.' { println!("{}", lit); i += 1; } else { let (targets, ni2) = extract_targets(code, i)?; i = ni2; for t in targets { vars.insert(t, Val::Str(lit.clone())); } } }
            if i < bytes.len() && (bytes[i] as char) == ';' { i += 1; }
            continue;
        } else if c == '*' {
            // loop: *N{...} or while: *?condition{...}
            i += 1;
            skip_ws_bytes(code.as_bytes(), &mut i);
            
            // check if it's a while loop (*? condition)
            if i < bytes.len() && (bytes[i] as char) == '?' {
                // while loop
                i += 1;
                skip_ws_bytes(code.as_bytes(), &mut i);
                // read until '{' and this is the condition
                let start_expr = i;
                while i < bytes.len() && (bytes[i] as char) != '{' { i += 1; }
                let cond_str = code[start_expr..i].trim();
                skip_ws_bytes(code.as_bytes(), &mut i);
                if i >= bytes.len() || (bytes[i] as char) != '{' { return Err("Expected '{' after while condition".into()); }
                let (block, ni2) = extract_braced_block(code, i)?;
                i = ni2;
                
                // while loop: keep executing block while condition is true
                let mut idx = 0;
                loop {
                    // keep _ as working
                    vars.insert("_".to_string(), Val::Int(idx as i64));
                    let cond_val = eval_expr(cond_str, vars)?;
                    if cond_val.as_i64() == 0 {
                        break;
                    }
                    run_block_simple_loop(&block, vars)?;
                    idx += 1;
                }
                continue;
            } else {
                // regular for loop: *N{...}
                // read until '{' and evaluate the expression
                let start_expr = i;
                while i < bytes.len() && (bytes[i] as char) != '{' { i += 1; }
                let expr_str = code[start_expr..i].trim();
                let num_val = eval_expr(expr_str, vars)?;
                let num = num_val.as_i64() as usize;
                skip_ws_bytes(code.as_bytes(), &mut i);
                if i >= bytes.len() || (bytes[i] as char) != '{' { return Err("Expected '{' after loop count".into()); }
                let (block, ni2) = extract_braced_block(code, i)?;
                i = ni2;
                for idx in 0..num {
                    vars.insert("_".to_string(), Val::Int(idx as i64));
                    run_block_simple_loop(&block, vars)?;
                }
                continue;
            }
        } else {
            // read until semicolon
            let start = i;
            while i < bytes.len() && (bytes[i] as char) != ';' { i += 1; }
            let stmt = &code[start..i];
            if i < bytes.len() && (bytes[i] as char) == ';' { i += 1; }
            if stmt.trim().is_empty() { continue; }
            exec_stmt(stmt, vars)?;
            continue;
        }
    }
    Ok(())
}

fn exec_stmt(stmt: &str, vars: &mut HashMap<String, Val>) -> Result<(), String> {
    let s = stmt.trim();
    if s.is_empty() { return Ok(()); }
    // find '>' (the core send operator)
    if let Some(pos) = s.find('>') {
        if pos == 0 { return Err(format!("Invalid statement: {}", s)); }
        let left = s[..pos].trim();
        // check if there is an augment operator immediately before '>' like +> or ^>
        let op_char = left.chars().last();
        let (expr_str, op) = if let Some(c) = op_char {
            if "+-*/^%".contains(c) {
                // augmented: expr then operator char is last char of left
                let mut left_chars = left.chars().collect::<Vec<_>>();
                left_chars.pop();
                (left_chars.into_iter().collect::<String>().trim().to_string(), Some(c))
            } else { (left.to_string(), None) }
        } else { (left.to_string(), None) };

        let right = s[pos + 1..].trim();
        // evaluate expression
        let val = eval_expr(expr_str.as_str(), vars)?;

        if right == "." {
            // print
            println!("{}", val.as_string());
            return Ok(());
        }

        // right side may be comma-separated targets
        let targets: Vec<&str> = right.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
        for t in targets {
            // target should be a variable name (letters)
            if t.is_empty() { continue; }
            if op.is_none() {
                // assignment
                vars.insert(t.to_string(), val.clone());
            } else {
                // augmented: variable = variable (op) value
                let opch = op.unwrap();
                let cur = vars.get(t).cloned().unwrap_or(Val::Int(0));
                let newv = match (cur, val.clone(), opch) {
                    (Val::Int(a), Val::Int(b), '+') => Val::Int(a + b),
                    (Val::Int(a), Val::Int(b), '-') => Val::Int(a - b),
                    (Val::Int(a), Val::Int(b), '*') => Val::Int(a * b),
                    (Val::Int(a), Val::Int(b), '/') => Val::Int(a / b),
                    (Val::Int(a), Val::Int(b), '%') => Val::Int(a % b),
                    (Val::Int(a), Val::Int(b), '^') => Val::Int(a.pow(b as u32)),
                    // append int to list
                    (Val::List(mut vec), Val::Int(b), '+') => {
                        vec.push(Val::Int(b));
                        Val::List(vec)
                    }
                    // fallback: try numeric
                    (Val::Str(sa), Val::Int(b), '+') => Val::Str(format!("{}{}", sa, b)),
                    _ => return Err(format!("Unsupported augmented op on types")),
                };
                vars.insert(t.to_string(), newv);
            }
        }
        Ok(())
    } else {
        Err(format!("No '>' operator found in statement: {}", s))
    }
}

fn eval_expr(s: &str, vars: &HashMap<String, Val>) -> Result<Val, String> {
    let expr = s.trim();
    if expr.is_empty() { return Ok(Val::Int(0)); }
    // macros: $a[b]
    if expr.starts_with('$') {
        // parse $name[arg]
        if let Some(br) = expr.find('[') {
            let name = &expr[1..br];
            if let Some(end) = expr.find(']') {
                let arg = &expr[br+1..end].trim();
                let val = vars.get(*arg).cloned().unwrap_or(Val::Int(0));
                match name {
                    "s" => {
                        // sum macro: sum the elements of a list
                        match val {
                            Val::List(items) => {
                                let sum: i64 = items.iter().map(|v| v.as_i64()).sum();
                                return Ok(Val::Int(sum));
                            }
                            Val::Int(n) => return Ok(Val::Int(n)),
                            Val::Str(st) => {
                                if let Ok(n) = st.parse::<i64>() {
                                    return Ok(Val::Int(n));
                                }
                                return Err(format!("Cannot sum string '{}': not a valid number", st));
                            }
                        }
                    }
                    "l" => {
                        // length macro: length of string or list
                        match val {
                            Val::List(items) => {
                                return Ok(Val::Int(items.len() as i64));
                            }
                            Val::Str(st) => {
                                return Ok(Val::Int(st.chars().count() as i64));
                            }
                            Val::Int(n) => {
                                return Err(format!("Cannot get length of integer '{}'", n));
                            }
                        }
                    }
                    _ => return Err(format!("Unknown macro: ${} (line with expression: {})", name, expr)),
                }
            } else {
                return Err(format!("Macro ${}[...] missing closing bracket ']'", name));
            }
        } else {
            return Err(format!("Macro expression '{}' missing opening bracket '['", expr));
        }
    }
    // list literal: ,[a,b,c]
    if expr.starts_with(',') {
        // expect ,[ ... ]
        let rest = expr[1..].trim();
        if rest.starts_with('[') && rest.ends_with(']') {
            let inner = &rest[1..rest.len()-1];
            if inner.trim().is_empty() {
                return Ok(Val::List(Vec::new()));
            }
            let mut items = Vec::new();
            for part in inner.split(',') {
                let p = part.trim();
                if p.is_empty() { continue; }
                // try parse number or string
                if p.starts_with('"') && p.ends_with('"') && p.len()>=2 {
                    items.push(Val::Str(p[1..p.len()-1].to_string()));
                } else {
                    let n: i64 = p.parse().map_err(|_| format!("Invalid list element '{}': expected integer or quoted string", p))?;
                    items.push(Val::Int(n));
                }
            }
            return Ok(Val::List(items));
        } else {
            return Err(format!("Invalid list literal '{}': expected format: ,[ item, item, ... ]", expr));
        }
    }
    // if expression is a string literal (rare here)
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        let inner = &expr[1..expr.len()-1];
        return Ok(Val::Str(inner.to_string()));
    }
    // Evaluate using a simple shunting-yard to RPN for integers
    // Tokenize with variable handling (variables and list indexing are resolved in tokenizer)
    let tokens = tokenize(expr, vars).map_err(|e| format!("In expression '{}': {}", expr, e))?;
    let rpn = to_rpn(tokens).map_err(|e| format!("In expression '{}': {}", expr, e))?;
    let v = eval_rpn(rpn).map_err(|e| format!("In expression '{}': {}", expr, e))?;
    Ok(Val::Int(v))
}

// Tokenizer
#[derive(Debug, Clone)]
enum Tok { Num(i64), Op(String) }

fn tokenize(s: &str, vars: &HashMap<String, Val>) -> Result<Vec<Tok>, String> {
    let mut i = 0usize;
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() { i += 1; continue; }
        // numbers (with optional decimal and exponent)
        if c.is_ascii_digit() {
            // support integer, decimal, and scientific notation (e.g., 1e6, 2.5e3)
            let start = i;
            let mut seen_e = false;
            let mut seen_dot = false;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_ascii_digit() {
                    i += 1; continue;
                }
                if (ch == 'e' || ch == 'E') && !seen_e {
                    seen_e = true;
                    i += 1;
                    // allow optional sign after exponent
                    if i < bytes.len() {
                        let nc = bytes[i] as char;
                        if nc == '+' || nc == '-' { i += 1; }
                    }
                    continue;
                }
                if ch == '.' && !seen_dot && !seen_e {
                    seen_dot = true;
                    i += 1; continue;
                }
                break;
            }
            let num_str = &s[start..i];
            // parse as float if it contains '.' or 'e'/'E', otherwise parse as integer
            let num: i64 = if num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
                let f: f64 = num_str.parse().map_err(|e| format!("Failed to parse float: {}", e))?;
                f as i64
            } else {
                num_str.parse().map_err(|e| format!("Failed to parse number: {}", e))?
            };
            out.push(Tok::Num(num));
            continue;
        }
        // variables and identifiers: letters, possibly followed by alphanumeric/_ and optional [index]
        if c.is_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_alphanumeric() || ch == '_' { i += 1; } else { break; }
            }
            let name = &s[start..i];
            // handle optional indexing like name[<number>]
            if i < bytes.len() && (bytes[i] as char) == '[' {
                i += 1; // consume '['
                let idx_start = i;
                while i < bytes.len() && (bytes[i] as char) != ']' { i += 1; }
                if i >= bytes.len() { return Err(format!("Unclosed '[' in variable indexing for '{}'", name)); }
                let idx_str = &s[idx_start..i];
                i += 1; // consume ']'
                let index: i64 = idx_str.trim().parse().unwrap_or(0);
                if let Some(Val::List(items)) = vars.get(name) {
                    let idx = if index < 0 { (items.len() as i64 + index) as usize } else { index as usize };
                    if idx < items.len() { out.push(Tok::Num(items[idx].as_i64())); } else { out.push(Tok::Num(0)); }
                } else { out.push(Tok::Num(0)); }
            } else {
                // plain variable
                out.push(Tok::Num(vars.get(name).map(|v| v.as_i64()).unwrap_or(0)));
            }
            continue;
        }
        // multi-char operators: ||, &&, <=, >=
        if i + 1 < bytes.len() && c == '|' && (bytes[i + 1] as char) == '|' {
            out.push(Tok::Op("||".to_string()));
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && c == '&' && (bytes[i + 1] as char) == '&' {
            out.push(Tok::Op("&&".to_string()));
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && c == '<' && (bytes[i + 1] as char) == '=' {
            out.push(Tok::Op("<=".to_string()));
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && c == '>' && (bytes[i + 1] as char) == '=' {
            out.push(Tok::Op(">=".to_string()));
            i += 2;
            continue;
        }
        // single-char operators and parentheses: < > = etc
        if "+-*/^()%<>=".contains(c) { out.push(Tok::Op(c.to_string())); i += 1; continue; }
        return Err(format!("Unexpected character '{}' in expression at position {}", c, i));
    }
    Ok(out)
}

fn prec(op: &str) -> i32 { 
    match op { 
        "^" => 4, 
        "*" | "/" | "%" => 3, 
        "+" | "-" => 2, 
        "=" | "<" | ">" | "<=" | ">=" => 2,
        "||" => 1,
        "&&" => 1,
        "(" | ")" => 0, 
        _ => 1 
    } 
}

fn to_rpn(tokens: Vec<Tok>) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut ops: Vec<String> = Vec::new();
    for t in tokens {
        match t {
            Tok::Num(n) => out.push(Tok::Num(n)),
            Tok::Op(ref op_str) if op_str == "(" => ops.push(op_str.clone()),
            Tok::Op(ref op_str) if op_str == ")" => {
                while let Some(op) = ops.pop() {
                    if op == "(" { break; }
                    out.push(Tok::Op(op));
                }
            },
            Tok::Op(op_str) => {
                while let Some(top) = ops.last() {
                    if (prec(top) > prec(&op_str)) || (prec(top) == prec(&op_str) && &op_str != "^") {
                        out.push(Tok::Op(top.clone())); 
                        ops.pop();
                    } else { break; }
                }
                ops.push(op_str);
            },
        }
    }
    while let Some(op) = ops.pop() { out.push(Tok::Op(op)); }
    Ok(out)
}

fn eval_rpn(rpn: Vec<Tok>) -> Result<i64, String> {
    let mut st: Vec<i64> = Vec::new();
    for t in rpn {
        match t {
            Tok::Num(n) => st.push(n),
            Tok::Op(op) => {
                let b = st.pop().ok_or(format!("Evaluation error: not enough operands for operator '{}'", op))?;
                let a = st.pop().ok_or(format!("Evaluation error: not enough operands for operator '{}'", op))?;
                let res = match op.as_str() {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" => {
                        if b == 0 {
                            return Err("Division by zero".to_string());
                        }
                        a / b
                    },
                    "%" => {
                        if b == 0 {
                            return Err("Modulo by zero".to_string());
                        }
                        a % b
                    },
                    "^" => a.pow(b as u32),
                    "=" => if a == b { 1 } else { 0 },
                    "<" => if a < b { 1 } else { 0 },
                    ">" => if a > b { 1 } else { 0 },
                    "<=" => if a <= b { 1 } else { 0 },
                    ">=" => if a >= b { 1 } else { 0 },
                    "||" => if a != 0 || b != 0 { 1 } else { 0 },
                    "&&" => if a != 0 && b != 0 { 1 } else { 0 },
                    _ => return Err(format!("Unknown operator: '{}'", op)),
                };
                st.push(res);
            },
        }
    }
    st.pop().ok_or("Evaluation error: empty expression result".into())
}

// Helpers for extraction in the simple runner
fn skip_ws_bytes(b: &[u8], i: &mut usize) { while *i < b.len() && (b[*i] as char).is_whitespace() { *i += 1; } }

fn count_newlines(s: &str) -> usize {
    s.chars().filter(|&c| c == '\n').count()
}

fn extract_string(s: &str, mut i: usize) -> Result<(String, usize), String> {
    let bytes = s.as_bytes();
    if bytes[i] as char != '"' { return Err("not a string".into()); }
    i += 1;
    let start = i;
    while i < bytes.len() && (bytes[i] as char) != '"' { i += 1; }
    if i >= bytes.len() { return Err("unterminated string".into()); }
    let lit = s[start..i].to_string();
    i += 1; // consume end quote
    Ok((lit, i))
}

fn extract_braced_block(s: &str, mut i: usize) -> Result<(String, usize), String> {
    let bytes = s.as_bytes();
    if bytes[i] as char != '{' { return Err("expected '{'".into()); }
    let open_line = count_newlines(&s[..i]);
    i += 1; // consume {
    let mut depth = 1usize;
    let start = i;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '{' { depth += 1; }
        else if c == '}' { depth -= 1; if depth == 0 { break; } }
        i += 1;
    }
    if i >= bytes.len() { 
        let current_line = count_newlines(&s[..i]);
        return Err(format!("unmatched '{{' at line {}, never closed (current line: {})", open_line + 1, current_line + 1)); 
    }
    let block = s[start..i].to_string();
    i += 1; // consume '}'
    Ok((block, i))
}

fn extract_targets(s: &str, mut i: usize) -> Result<(Vec<String>, usize), String> {
    let bytes = s.as_bytes();
    skip_ws_bytes(bytes, &mut i);
    let start = i;
    while i < bytes.len() && (bytes[i] as char) != ';' && (bytes[i] as char) != '\n' { i += 1; }
    let raw = s[start..i].trim();
    let targets = raw.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    Ok((targets, i))
}

fn handle_if_chain(code: &str, i: &mut usize, vars: &mut HashMap<String, Val>) -> Result<(), String> {
    let bytes = code.as_bytes();
    let mut matched = false;
    loop {
        skip_ws_bytes(bytes, i);
        if *i >= bytes.len() { break; }
        // determine clause type
        let clause = if bytes[*i] as char == '?' {
            *i += 1;
            "if"
        } else if bytes[*i] as char == '!' {
            if *i + 1 < bytes.len() && (bytes[*i + 1] as char) == '?' {
                *i += 2; "elif"
            } else if *i + 1 < bytes.len() && (bytes[*i + 1] as char) == '!' {
                *i += 2; "else"
            } else { return Err("Invalid if-clause".into()); }
        } else { break; };

        skip_ws_bytes(bytes, i);
        let truth = if clause != "else" {
            // read until '{' as expression
            let start_expr = *i;
            while *i < bytes.len() && (bytes[*i] as char) != '{' { *i += 1; }
            let expr_str = code[start_expr..*i].trim();
            let val = eval_expr(expr_str, vars)?;
            val.as_i64() != 0
        } else { true };

        skip_ws_bytes(bytes, i);
        if *i >= bytes.len() || (bytes[*i] as char) != '{' { return Err("Expected '{' after if condition".into()); }
        let (block, ni2) = extract_braced_block(code, *i)?;
        *i = ni2;
        if !matched && truth {
            run_block_simple_loop(&block, vars)?;
            matched = true;
        }

        // peek for next clause: skip whitespace and check next char
        skip_ws_bytes(bytes, i);
        if *i >= bytes.len() { break; }
        let nextc = bytes[*i] as char;
        if !(nextc == '?' || (nextc == '!' && *i + 1 < bytes.len() && ((bytes[*i+1] as char)=='?' || (bytes[*i+1] as char)=='!'))) {
            break;
        }
    }
    Ok(())
}
