//! SPICE parameter expressions: `{...}` / `'...'` values, `.param` definitions (chained and forward references)
//! and `.func` user functions.
//!
//! Grammar (lowest to highest precedence): `c ? a : b`, `||`, `&&`, `== !=`, `< <= > >=`, `+ -`, `* / %`,
//! unary `+ - !`, `^` / `**` (right associative), primaries: numbers with SPICE scale suffixes (`1k`, `2.5meg`,
//! `10p`), identifiers (parameters), function calls and parentheses.
//!
//! Every evaluation is bounded (Rule 9): expressions nest at most [`MAX_DEPTH`] levels, user functions call each
//! other at most [`MAX_CALL_DEPTH`] deep, and `.param` chains are ordered by an O(V + E) topological sort with
//! cycle detection instead of recursive on-demand evaluation.

use rustc_hash::{FxHashMap, FxHashSet};

/// Deepest nesting of parentheses / operators in one expression.
pub const MAX_DEPTH: usize = 256;
/// Deepest chain of user-function calls (a recursive `.func` stops here).
pub const MAX_CALL_DEPTH: usize = 64;

/// A user function from `.func name(a, b) {body}`.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncDef {
    pub params: Vec<String>,
    pub body: String,
}

/// Parameter values visible to an expression: the circuit-wide table plus the overrides of the subcircuit instance
/// being expanded (looked up first).
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub globals: &'a FxHashMap<String, f64>,
    pub locals: Option<&'a FxHashMap<String, f64>>,
    pub funcs: &'a FxHashMap<String, FuncDef>,
}

impl<'a> Scope<'a> {
    pub fn lookup(&self, name: &str) -> Option<f64> {
        if let Some(v) = self.locals.and_then(|l| l.get(name)) {
            return Some(*v);
        }
        self.globals.get(name).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(&'static str),
}

/// SPICE scale factor of the letters after a number (`meg`, `mil`, `t g k m u n p f a`; anything else is 1).
pub fn scale_suffix(suffix: &str) -> f64 {
    let s = suffix.as_bytes();
    if s.is_empty() {
        return 1.0;
    }
    let lower = |i: usize| s.get(i).map(|b| b.to_ascii_lowercase());
    if lower(0) == Some(b'm') && lower(1) == Some(b'e') && lower(2) == Some(b'g') {
        return 1e6;
    }
    if lower(0) == Some(b'm') && lower(1) == Some(b'i') && lower(2) == Some(b'l') {
        return 25.4e-6;
    }
    match lower(0) {
        Some(b'x') => 1e6,
        Some(b't') => 1e12,
        Some(b'g') => 1e9,
        Some(b'k') => 1e3,
        Some(b'm') => 1e-3,
        Some(b'u') => 1e-6,
        Some(b'n') => 1e-9,
        Some(b'p') => 1e-12,
        Some(b'f') => 1e-15,
        Some(b'a') => 1e-18,
        _ => 1.0,
    }
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                j += 1;
            }
            if j < b.len()
                && (b[j] == b'e' || b[j] == b'E')
                && (b.get(j + 1).is_some_and(u8::is_ascii_digit)
                    || (matches!(b.get(j + 1), Some(b'+' | b'-'))
                        && b.get(j + 2).is_some_and(u8::is_ascii_digit)))
            {
                j += 2;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
            }
            let base: f64 = src[i..j]
                .parse()
                .map_err(|_| format!("bad number `{}`", &src[i..j]))?;
            let k = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            out.push(Tok::Num(base * scale_suffix(&src[k..j])));
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            out.push(Tok::Ident(src[i..j].to_ascii_lowercase()));
            i = j;
            continue;
        }
        let two = src.get(i..i + 2).unwrap_or("");
        let op: &'static str = match two {
            "**" => "^",
            "==" => "==",
            "!=" => "!=",
            "<=" => "<=",
            ">=" => ">=",
            "&&" => "&&",
            "||" => "||",
            _ => "",
        };
        if !op.is_empty() {
            out.push(Tok::Op(op));
            i += 2;
            continue;
        }
        let op: &'static str = match c {
            b'+' => "+",
            b'-' => "-",
            b'*' => "*",
            b'/' => "/",
            b'%' => "%",
            b'^' => "^",
            b'(' => "(",
            b')' => ")",
            b',' => ",",
            b'?' => "?",
            b':' => ":",
            b'<' => "<",
            b'>' => ">",
            b'!' => "!",
            _ => return Err(format!("unexpected character `{}`", c as char)),
        };
        out.push(Tok::Op(op));
        i += 1;
    }
    Ok(out)
}

struct Parser<'s, 'a> {
    toks: &'s [Tok],
    pos: usize,
    scope: Scope<'a>,
    /// Arguments of the user function being evaluated (looked up before the scope).
    args: Option<&'s FxHashMap<String, f64>>,
    call_depth: usize,
}

impl Parser<'_, '_> {
    fn peek_op(&self) -> Option<&'static str> {
        match self.toks.get(self.pos) {
            Some(Tok::Op(o)) => Some(o),
            _ => None,
        }
    }

    fn eat(&mut self, op: &str) -> bool {
        if self.peek_op() == Some(op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, op: &str) -> Result<(), String> {
        if self.eat(op) {
            Ok(())
        } else {
            Err(format!("expected `{op}`"))
        }
    }

    fn ternary(&mut self, depth: usize) -> Result<f64, String> {
        if depth > MAX_DEPTH {
            return Err(format!("expression nested deeper than {MAX_DEPTH} levels"));
        }
        let c = self.binary(0, depth + 1)?;
        if self.eat("?") {
            let a = self.ternary(depth + 1)?;
            self.expect(":")?;
            let b = self.ternary(depth + 1)?;
            return Ok(if c != 0.0 { a } else { b });
        }
        Ok(c)
    }

    /// Precedence climbing over the binary operator levels.
    fn binary(&mut self, level: usize, depth: usize) -> Result<f64, String> {
        const LEVELS: [&[&str]; 6] = [
            &["||"],
            &["&&"],
            &["==", "!="],
            &["<", "<=", ">", ">="],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if depth > MAX_DEPTH {
            return Err(format!("expression nested deeper than {MAX_DEPTH} levels"));
        }
        if level == LEVELS.len() {
            return self.unary(depth + 1);
        }
        let mut lhs = self.binary(level + 1, depth + 1)?;
        while let Some(op) = self.peek_op().filter(|o| LEVELS[level].contains(o)) {
            self.pos += 1;
            let rhs = self.binary(level + 1, depth + 1)?;
            let t = |b: bool| if b { 1.0 } else { 0.0 };
            lhs = match op {
                "||" => t(lhs != 0.0 || rhs != 0.0),
                "&&" => t(lhs != 0.0 && rhs != 0.0),
                "==" => t(lhs == rhs),
                "!=" => t(lhs != rhs),
                "<" => t(lhs < rhs),
                "<=" => t(lhs <= rhs),
                ">" => t(lhs > rhs),
                ">=" => t(lhs >= rhs),
                "+" => lhs + rhs,
                "-" => lhs - rhs,
                "*" => lhs * rhs,
                "/" => lhs / rhs,
                _ => lhs % rhs,
            };
        }
        Ok(lhs)
    }

    fn unary(&mut self, depth: usize) -> Result<f64, String> {
        if depth > MAX_DEPTH {
            return Err(format!("expression nested deeper than {MAX_DEPTH} levels"));
        }
        if self.eat("-") {
            return Ok(-self.unary(depth + 1)?);
        }
        if self.eat("+") {
            return self.unary(depth + 1);
        }
        if self.eat("!") {
            return Ok(if self.unary(depth + 1)? == 0.0 {
                1.0
            } else {
                0.0
            });
        }
        self.power(depth + 1)
    }

    fn power(&mut self, depth: usize) -> Result<f64, String> {
        let base = self.primary(depth + 1)?;
        if self.eat("^") {
            // right associative and binding tighter than a unary minus on its left: -2^2 = -4, 2^-1 = 0.5
            let exp = self.unary(depth + 1)?;
            return Ok(base.powf(exp));
        }
        Ok(base)
    }

    fn primary(&mut self, depth: usize) -> Result<f64, String> {
        if depth > MAX_DEPTH {
            return Err(format!("expression nested deeper than {MAX_DEPTH} levels"));
        }
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Num(v)) => {
                self.pos += 1;
                Ok(v)
            }
            Some(Tok::Op("(")) => {
                self.pos += 1;
                let v = self.ternary(depth + 1)?;
                self.expect(")")?;
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                if self.eat("(") {
                    let mut args = Vec::new();
                    if !self.eat(")") {
                        loop {
                            args.push(self.ternary(depth + 1)?);
                            if self.eat(")") {
                                break;
                            }
                            self.expect(",")?;
                        }
                    }
                    return self.call(&name, &args);
                }
                if let Some(v) = self.args.and_then(|a| a.get(&name)) {
                    return Ok(*v);
                }
                if let Some(v) = self.scope.lookup(&name) {
                    return Ok(v);
                }
                match name.as_str() {
                    "pi" => Ok(std::f64::consts::PI),
                    "e" => Ok(std::f64::consts::E),
                    _ => Err(format!("unknown parameter `{name}`")),
                }
            }
            Some(Tok::Op(o)) => Err(format!("unexpected `{o}`")),
            None => Err("unexpected end of expression".to_string()),
        }
    }

    fn call(&mut self, name: &str, a: &[f64]) -> Result<f64, String> {
        let arity = |n: usize| -> Result<(), String> {
            if a.len() == n {
                Ok(())
            } else {
                Err(format!("{name}() takes {n} argument(s), got {}", a.len()))
            }
        };
        let f1 = |f: fn(f64) -> f64| -> Result<f64, String> {
            arity(1)?;
            Ok(f(a[0]))
        };
        match name {
            "abs" => f1(f64::abs),
            "sqrt" => f1(f64::sqrt),
            "exp" => f1(f64::exp),
            "ln" | "log" => f1(f64::ln),
            "log10" => f1(f64::log10),
            "sin" => f1(f64::sin),
            "cos" => f1(f64::cos),
            "tan" => f1(f64::tan),
            "asin" | "arcsin" => f1(f64::asin),
            "acos" | "arccos" => f1(f64::acos),
            "atan" | "arctan" => f1(f64::atan),
            "sinh" => f1(f64::sinh),
            "cosh" => f1(f64::cosh),
            "tanh" => f1(f64::tanh),
            "asinh" => f1(f64::asinh),
            "acosh" => f1(f64::acosh),
            "atanh" => f1(f64::atanh),
            "floor" => f1(f64::floor),
            "ceil" => f1(f64::ceil),
            "int" => f1(f64::trunc),
            "nint" | "round" => f1(f64::round),
            "sgn" | "sign" => f1(|x| {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }),
            "u" | "step" => f1(|x| if x > 0.0 { 1.0 } else { 0.0 }),
            "uramp" => f1(|x| x.max(0.0)),
            "min" | "max" => {
                if a.is_empty() {
                    return Err(format!("{name}() needs arguments"));
                }
                let pick = if name == "min" { f64::min } else { f64::max };
                Ok(a[1..].iter().fold(a[0], |acc, &v| pick(acc, v)))
            }
            "pow" => {
                arity(2)?;
                Ok(a[0].powf(a[1]))
            }
            "pwr" => {
                arity(2)?;
                Ok(a[0].abs().powf(a[1]) * a[0].signum())
            }
            "pwrs" => {
                arity(2)?;
                Ok(a[0].abs().powf(a[1]) * if a[0] < 0.0 { -1.0 } else { 1.0 })
            }
            "atan2" => {
                arity(2)?;
                Ok(a[0].atan2(a[1]))
            }
            "hypot" => {
                arity(2)?;
                Ok(a[0].hypot(a[1]))
            }
            "if" | "ternary_fcn" => {
                arity(3)?;
                Ok(if a[0] != 0.0 { a[1] } else { a[2] })
            }
            "limit" => {
                arity(3)?;
                let (lo, hi) = if a[1] <= a[2] {
                    (a[1], a[2])
                } else {
                    (a[2], a[1])
                };
                Ok(a[0].clamp(lo, hi))
            }
            _ => {
                let Some(def) = self.scope.funcs.get(name) else {
                    return Err(format!("unknown function `{name}`"));
                };
                if def.params.len() != a.len() {
                    return Err(format!(
                        "{name}() takes {} argument(s), got {}",
                        def.params.len(),
                        a.len()
                    ));
                }
                if self.call_depth >= MAX_CALL_DEPTH {
                    return Err(format!(
                        "user functions nest deeper than {MAX_CALL_DEPTH} calls (recursive .func `{name}`?)"
                    ));
                }
                let bound: FxHashMap<String, f64> =
                    def.params.iter().cloned().zip(a.iter().copied()).collect();
                let toks = lex(&def.body)?;
                let mut p = Parser {
                    toks: &toks,
                    pos: 0,
                    scope: self.scope,
                    args: Some(&bound),
                    call_depth: self.call_depth + 1,
                };
                let v = p.ternary(0)?;
                if p.pos != toks.len() {
                    return Err(format!("trailing text in the body of .func {name}"));
                }
                Ok(v)
            }
        }
    }
}

/// Strips one level of `{...}` or `'...'` around an expression.
pub fn unwrap_delimiters(src: &str) -> &str {
    let t = src.trim();
    if t.len() >= 2
        && ((t.starts_with('{') && t.ends_with('}')) || (t.starts_with('\'') && t.ends_with('\'')))
    {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// Evaluates `src` (with or without its `{}` / `''` delimiters). A non-finite result (overflow, division by zero,
/// `sqrt(-1)`) is an error, as in SPICE.
pub fn eval(src: &str, scope: Scope<'_>) -> Result<f64, String> {
    let body = unwrap_delimiters(src);
    let toks = lex(body)?;
    if toks.is_empty() {
        return Err("empty expression".to_string());
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        scope,
        args: None,
        call_depth: 0,
    };
    let v = p.ternary(0)?;
    if p.pos != toks.len() {
        return Err(format!("unexpected text after the expression `{body}`"));
    }
    if !v.is_finite() {
        return Err(format!("`{body}` evaluates to {v}"));
    }
    Ok(v)
}

/// Parameter names an expression refers to (identifiers that are not function calls).
fn references(src: &str) -> Vec<String> {
    let Ok(toks) = lex(unwrap_delimiters(src)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if let Tok::Ident(name) = t {
            if toks.get(i + 1) != Some(&Tok::Op("(")) {
                out.push(name.clone());
            }
        }
    }
    out
}

/// Evaluates `.param` definitions `(name, expression)` in dependency order (a definition may refer to one written
/// later), writing every value into `table` (which may already hold values that the definitions can use). The last
/// definition of a name wins. Returns `(name, reason)` for every definition that could not be evaluated: an unknown
/// name, a syntax error, a non-finite value or a dependency cycle.
pub fn resolve_params(
    defs: &[(String, String)],
    table: &mut FxHashMap<String, f64>,
    funcs: &FxHashMap<String, FuncDef>,
) -> Vec<(String, String)> {
    resolve_params_in(defs, table, None, funcs)
}

/// As [`resolve_params`], with `fallback` consulted for names `table` does not hold (the circuit-wide parameters
/// while the parameters of a subcircuit instance are resolved into `table`).
pub fn resolve_params_in(
    defs: &[(String, String)],
    table: &mut FxHashMap<String, f64>,
    fallback: Option<&FxHashMap<String, f64>>,
    funcs: &FxHashMap<String, FuncDef>,
) -> Vec<(String, String)> {
    // last definition wins
    let mut index: FxHashMap<&str, usize> = FxHashMap::default();
    for (i, (name, _)) in defs.iter().enumerate() {
        index.insert(name.as_str(), i);
    }
    let live: Vec<usize> = {
        let mut v: Vec<usize> = index.values().copied().collect();
        v.sort_unstable();
        v
    };
    // dependency graph among the live definitions (Kahn's algorithm, O(V + E))
    let mut indeg: FxHashMap<usize, usize> = live.iter().map(|&i| (i, 0)).collect();
    let mut users: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    for &i in &live {
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        for r in references(&defs[i].1) {
            if let Some(&j) = index.get(r.as_str()) {
                if j != i && seen.insert(j) {
                    *indeg.get_mut(&i).unwrap() += 1;
                    users.entry(j).or_default().push(i);
                }
            }
        }
    }
    let mut ready: Vec<usize> = live.iter().copied().filter(|i| indeg[i] == 0).collect();
    ready.reverse();
    let mut failed = Vec::new();
    let mut done = 0usize;
    while let Some(i) = ready.pop() {
        done += 1;
        let (name, src) = &defs[i];
        let scope = match fallback {
            Some(g) => Scope {
                globals: g,
                locals: Some(table),
                funcs,
            },
            None => Scope {
                globals: table,
                locals: None,
                funcs,
            },
        };
        match eval(src, scope) {
            Ok(v) => {
                table.insert(name.clone(), v);
            }
            Err(e) => failed.push((name.clone(), e)),
        }
        if let Some(us) = users.get(&i) {
            let mut next: Vec<usize> = Vec::new();
            for &u in us {
                let d = indeg.get_mut(&u).unwrap();
                *d -= 1;
                if *d == 0 {
                    next.push(u);
                }
            }
            next.sort_unstable_by(|a, b| b.cmp(a));
            ready.extend(next);
        }
    }
    if done < live.len() {
        for &i in &live {
            if indeg[&i] > 0 {
                failed.push((
                    defs[i].0.clone(),
                    "the definition depends on itself (parameter cycle)".to_string(),
                ));
            }
        }
    }
    failed
}

/// Splits the text after `.param` (or a `PARAMS:` list) into `(name, expression)` assignments. Blanks around `=`
/// are allowed; an expression ends where the next `name=` begins (outside braces, quotes and parentheses).
pub fn split_assignments(text: &str) -> Result<Vec<(String, String)>, String> {
    let b = text.as_bytes();
    // positions of the assignment `=` signs at nesting depth 0
    let mut eqs: Vec<usize> = Vec::new();
    let mut depth = 0i32;
    let mut quote = false;
    for i in 0..b.len() {
        let c = b[i];
        if c == b'\'' {
            quote = !quote;
            continue;
        }
        if quote {
            continue;
        }
        match c {
            b'{' | b'(' => depth += 1,
            b'}' | b')' => depth -= 1,
            b'=' if depth == 0 => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if !matches!(prev, b'=' | b'!' | b'<' | b'>') && next != b'=' {
                    eqs.push(i);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    let mut name_spans: Vec<(usize, usize)> = Vec::new();
    for &e in &eqs {
        let mut j = e;
        while j > 0 && b[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        let end = j;
        while j > 0 && (b[j - 1].is_ascii_alphanumeric() || b[j - 1] == b'_') {
            j -= 1;
        }
        if j == end {
            return Err(format!("`=` without a parameter name in `{}`", text.trim()));
        }
        name_spans.push((j, end));
    }
    if let Some(&(first, _)) = name_spans.first() {
        if !text[..first].trim().is_empty() {
            return Err(format!(
                "`{}` is not a `name=value` assignment",
                text[..first].trim()
            ));
        }
    }
    for (k, &e) in eqs.iter().enumerate() {
        let (ns, ne) = name_spans[k];
        let vend = name_spans.get(k + 1).map_or(b.len(), |n| n.0);
        let value = text[e + 1..vend].trim();
        if value.is_empty() {
            return Err(format!("parameter `{}` has no value", &text[ns..ne]));
        }
        out.push((text[ns..ne].to_ascii_lowercase(), value.to_string()));
    }
    Ok(out)
}

/// Replaces every `{...}` and `'...'` expression of a card by its value (shortest round-trip literal). Text
/// without delimiters is returned unchanged (no allocation of a new string is needed by the caller then).
pub fn substitute(line: &str, scope: Scope<'_>) -> Result<String, String> {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        let c = b[i];
        let end = match c {
            b'{' => {
                let mut depth = 0i32;
                let mut j = i;
                loop {
                    match b.get(j) {
                        Some(b'{') => depth += 1,
                        Some(b'}') => {
                            depth -= 1;
                            if depth == 0 {
                                break Some(j);
                            }
                        }
                        None => break None,
                        _ => {}
                    }
                    j += 1;
                }
            }
            b'\'' => line[i + 1..].find('\'').map(|k| i + 1 + k),
            _ => {
                i += 1;
                continue;
            }
        };
        let Some(end) = end else {
            return Err(format!("unbalanced `{}` in `{}`", c as char, line.trim()));
        };
        let v = eval(&line[i..=end], scope)?;
        out.push_str(&line[last..i]);
        out.push_str(&format!("{v:e}"));
        i = end + 1;
        last = i;
    }
    out.push_str(&line[last..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(src: &str, params: &[(&str, f64)]) -> Result<f64, String> {
        let g: FxHashMap<String, f64> = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let f = FxHashMap::default();
        eval(
            src,
            Scope {
                globals: &g,
                locals: None,
                funcs: &f,
            },
        )
    }

    #[test]
    fn arithmetic_precedence_and_suffixes() {
        assert_eq!(ev("{1k+2k*3}", &[]).unwrap(), 7000.0);
        assert_eq!(ev("{10k/2/5}", &[]).unwrap(), 1000.0);
        assert_eq!(ev("{-(-1k)}", &[]).unwrap(), 1000.0);
        assert_eq!(ev("{2^3^2}", &[]).unwrap(), 512.0);
        assert_eq!(ev("{-2**2}", &[]).unwrap(), -4.0);
        assert_eq!(ev("'rv*2'", &[("rv", 1.5e3)]).unwrap(), 3e3);
        assert!((ev("{2.5meg}", &[]).unwrap() - 2.5e6).abs() < 1e-6);
        assert!((ev("{1u*5}", &[]).unwrap() - 5e-6).abs() < 1e-18);
        assert_eq!(ev("{ sel > 1 ? 1k : 2k }", &[("sel", 2.0)]).unwrap(), 1e3);
        assert_eq!(ev("{sqrt(16)*1k}", &[]).unwrap(), 4e3);
        assert_eq!(ev("{exp(0)*1k}", &[]).unwrap(), 1e3);
        assert_eq!(ev("{max(1, 3, 2)}", &[]).unwrap(), 3.0);
        assert_eq!(ev("{pwr(-2, 2)}", &[]).unwrap(), -4.0);
        assert_eq!(ev("{if(0, 1, 2)}", &[]).unwrap(), 2.0);
        let deep = format!("{{{}1k{}}}", "(".repeat(20), ")".repeat(20));
        assert_eq!(ev(&deep, &[]).unwrap(), 1e3);
    }

    #[test]
    fn errors_are_reported() {
        assert!(ev("{1e308*10}", &[]).unwrap_err().contains("inf"));
        assert!(ev("{1/z}", &[("z", 0.0)]).is_err());
        assert!(ev("{nope*2}", &[])
            .unwrap_err()
            .contains("unknown parameter"));
        assert!(ev("{1+}", &[]).is_err());
        assert!(ev("{(1}", &[]).is_err());
        let deep = format!("{{{}1{}}}", "(".repeat(400), ")".repeat(400));
        assert!(ev(&deep, &[]).unwrap_err().contains("nested deeper"));
    }

    #[test]
    fn param_chains_forward_references_and_cycles() {
        let defs: Vec<(String, String)> = (0..2000)
            .map(|i| {
                if i == 0 {
                    ("p0".to_string(), "1k".to_string())
                } else {
                    (format!("p{i}"), format!("{{1+p{}}}", i - 1))
                }
            })
            .rev()
            .collect();
        let mut t = FxHashMap::default();
        let failed = resolve_params(&defs, &mut t, &FxHashMap::default());
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(t["p1999"], 2999.0);
        let cyc = vec![
            ("a".to_string(), "{b+1}".to_string()),
            ("b".to_string(), "{a+1}".to_string()),
            ("c".to_string(), "3".to_string()),
        ];
        let mut t = FxHashMap::default();
        let failed = resolve_params(&cyc, &mut t, &FxHashMap::default());
        assert_eq!(failed.len(), 2);
        assert_eq!(t["c"], 3.0);
    }

    #[test]
    fn assignments_and_substitution() {
        let a = split_assignments(" ra = 3k rb={ra*2}  rc='1+ 2'").unwrap();
        assert_eq!(
            a,
            vec![
                ("ra".to_string(), "3k".to_string()),
                ("rb".to_string(), "{ra*2}".to_string()),
                ("rc".to_string(), "'1+ 2'".to_string())
            ]
        );
        assert!(split_assignments("junk a=1").is_err());
        let g: FxHashMap<String, f64> = [("rv".to_string(), 1e3)].into_iter().collect();
        let mut funcs = FxHashMap::default();
        funcs.insert(
            "dbl".to_string(),
            FuncDef {
                params: vec!["x".to_string()],
                body: "2*x".to_string(),
            },
        );
        let s = Scope {
            globals: &g,
            locals: None,
            funcs: &funcs,
        };
        assert_eq!(substitute("R1 a b { rv * 2 }", s).unwrap(), "R1 a b 2e3");
        assert_eq!(substitute("R1 a b {dbl(1k)}", s).unwrap(), "R1 a b 2e3");
        assert_eq!(substitute("R1 a b 1k", s).unwrap(), "R1 a b 1k");
        assert!(substitute("R1 a b {rv", s).is_err());
    }
}
