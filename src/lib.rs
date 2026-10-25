//! Zangyo: a tiny, deliberately over-serious joke compiler.
use sashimi_assembler::{
    Target,
    program::{assemble_program, render_asm, EntryKind, Linkage, OriginId, Program, Provenance, Section},
    typed::{Instruction, Opcode, Operand, Register, Width},
};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub message: String,
}
impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}行目: {}", self.line, self.message)
    }
}
impl Error for Issue {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Report { text: String, line: usize },
    Overtime { body: Vec<Statement>, line: usize },
    Leave { line: usize },
    Vacation { line: usize },
    Collapse { line: usize },
}
impl Statement {
    fn line(&self) -> usize {
        match self {
            Self::Report { line, .. } | Self::Overtime { line, .. }
            | Self::Leave { line } | Self::Vacation { line }
            | Self::Collapse { line } => *line,
        }
    }
    fn terminal(&self) -> bool {
        matches!(self, Self::Overtime { .. } | Self::Leave { .. } | Self::Collapse { .. })
    }
}

pub struct Parsed {
    pub statements: Vec<Statement>,
    pub warnings: Vec<String>,
}

fn parse_string(s: &str, line: usize) -> Result<String, Issue> {
    let s = s.trim();
    let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) else {
        return Err(Issue { line, message: "報告には引用符で囲った文字列が必要です".into() });
    };
    let mut out = String::new();
    let mut iter = inner.chars();
    while let Some(c) = iter.next() {
        if c == '"' {
            return Err(Issue { line, message: "文字列内の引用符は \\" とエスケープしてください".into() });
        }
        let next = if c == '\\' {
            match iter.next() {
                Some('n') => '\n', Some('t') => '\t', Some('r') => '\r',
                Some('"') => '"', Some('\\') => '\\',
                _ => return Err(Issue { line, message: "不正なエスケープです".into() }),
            }
        } else { c };
        if next == '\0' {
            return Err(Issue { line, message: "NUL文字を報告することはできません".into() });
        }
        out.push(next);
    }
    Ok(out)
}

fn block(lines: &[(usize, &str)], pos: &mut usize, nested: bool) -> Result<Vec<Statement>, Issue> {
    let mut nodes = vec![];
    while *pos < lines.len() {
        let (line, text) = lines[*pos];
        *pos += 1;
        if text == "}" {
            if nested { return Ok(nodes); }
            return Err(Issue { line, message: "対応する残業のない } です".into() });
        }
        let stmt = match text {
            "残業 {" => Statement::Overtime { body: block(lines, pos, true)?, line },
            "定時退社" => Statement::Leave { line },
            "有給" => Statement::Vacation { line },
            "過労死" => Statement::Collapse { line },
            _ if text.starts_with("報告 ") => Statement::Report {
                text: parse_string(text.strip_prefix("報告 ").unwrap(), line)?, line,
            },
            _ => return Err(Issue { line, message: format!("不明な業務命令: {text}") }),
        };
        nodes.push(stmt);
    }
    if nested {
        return Err(Issue { line: lines.last().map_or(1, |x| x.0), message: "残業ブロックの } がありません".into() });
    }
    Ok(nodes)
}
fn warnings(nodes: &[Statement], messages: &mut Vec<String>) {
    let mut finished = false;
    for stmt in nodes {
        if finished {
            messages.push(format!("warning[Z001]: {}行目の処理には到達できません (仕様です)", stmt.line()));
        }
        if let Statement::Overtime { body, .. } = stmt { warnings(body, messages); }
        if stmt.terminal() { finished = true; }
    }
}

pub fn parse(source: &str) -> Result<Parsed, Issue> {
    let lines: Vec<_> = source.lines().enumerate()
        .map(|(i, s)| (i + 1, s.trim()))
        .filter(|(_, s)| !s.is_empty() && !s.starts_with("//"))
        .collect();
    let Some((_, "出勤")) = lines.first().copied() else {
        return Err(Issue { line: lines.first().map_or(1, |x| x.0), message: "最初に出勤してください".into() });
    };
    let mut at = 0;
    let statements = block(&lines[1..], &mut at, false)?;
    let mut messages = vec![];
    warnings(&statements, &mut messages);
    Ok(Parsed { statements, warnings: messages })
}

pub struct Output {
    pub object: Vec<u8>,
    pub asm: String,
    pub origins: String,
    pub warnings: Vec<String>,
}

fn emit(p: &mut Program, opcode: Opcode, operands: Vec<Operand>, line: Option<usize>) {
    let provenance = line.map_or_else(Provenance::synthetic, |n| Provenance::source(OriginId(n as u64)));
    p.push(EntryKind::Instruction(Instruction::new(opcode, operands)), provenance);
}

fn lower(
    stmts: &[Statement], p: &mut Program, target: Target, report_symbols: &mut impl Iterator<Item = sashimi_assembler::program::SymbolId>,
    puts: sashimi_assembler::program::SymbolId,
) -> Result<(), Box<dyn Error>> {
    let rsp: Operand = Register::gpr(4, Width::Qword)?.into();
    let arg: Operand = Register::gpr(if target == Target::X86_64PcWindowsMsvc { 1 } else { 7 }, Width::Qword)?.into();
    let eax: Operand = Register::gpr(0, Width::Dword)?.into();
    for stmt in stmts {
        match stmt {
            Statement::Report { line, .. } => {
                let sym = report_symbols.next().expect("a symbol for each report");
                emit(p, Opcode::Lea64, vec![Operand::RipRelative(sym), arg.clone()], Some(*line));
                // Windows x64: 32-byte shadow space + alignment; SysV: align stack to 16 before call.
                emit(p, Opcode::Sub64, vec![Operand::Immediate(if target == Target::X86_64PcWindowsMsvc { 40 } else { 8 }), rsp.clone()], Some(*line));
                emit(p, Opcode::Call, vec![Operand::Symbol(puts)], Some(*line));
                emit(p, Opcode::Add64, vec![Operand::Immediate(if target == Target::X86_64PcWindowsMsvc { 40 } else { 8 }), rsp.clone()], Some(*line));
            }
            Statement::Leave { line } => {
                emit(p, Opcode::Xor32, vec![eax.clone(), eax.clone()], Some(*line));
                emit(p, Opcode::Ret, vec![], Some(*line));
            }
            Statement::Vacation { line } => {
                p.push(EntryKind::Marker, Provenance::source(OriginId(*line as u64)));
            }
            Statement::Collapse { line } => emit(p, Opcode::Undefined, vec![], Some(*line)),
            Statement::Overtime { body, line } => {
                let label = p.declare(format!(".Lzangyo_{}", p.symbols.len()), Linkage::Local);
                p.push(EntryKind::Label(label), Provenance::source(OriginId(*line as u64)));
                lower(body, p, target, report_symbols, puts)?;
                emit(p, Opcode::Jump, vec![Operand::Symbol(label)], Some(*line));
            }
        }
    }
    Ok(())
}

fn gather(stmts: &[Statement], out: &mut Vec<(usize, String)>) {
    for stmt in stmts {
        match stmt {
            Statement::Report { text, line } => out.push((*line, text.clone())),
            Statement::Overtime { body, .. } => gather(body, out),
            _ => {}
        }
    }
}

pub fn compile(source: &str, target: Target) -> Result<Output, Box<dyn Error>> {
    let parsed = parse(source)?;
    let mut program = Program::new(target);
    let main = program.declare("main", Linkage::Global);
    let mut strings = vec![];
    gather(&parsed.statements, &mut strings);
    let mut reports = vec![];
    for (i, _) in strings.iter().enumerate() {
        reports.push(program.declare(format!(".Lreport_{i}"), Linkage::Local));
    }
    let puts = program.declare("puts", Linkage::External);
    if !strings.is_empty() {
        program.push(EntryKind::Section(Section::ReadOnlyData), Provenance::synthetic());
        for ((line, value), symbol) in strings.iter().zip(reports.iter()) {
            program.push(EntryKind::Label(*symbol), Provenance::source(OriginId(*line as u64)));
            program.push(EntryKind::CString(value.clone()), Provenance::source(OriginId(*line as u64)));
        }
    }
    program.push(EntryKind::Section(Section::Text), Provenance::synthetic());
    program.push(EntryKind::Label(main), Provenance::synthetic());
    let mut report_symbols = reports.into_iter();
    lower(&parsed.statements, &mut program, target, &mut report_symbols, puts)?;
    // Unreachable when a terminal statement precedes it; harmless otherwise.
    let eax: Operand = Register::gpr(0, Width::Dword)?.into();
    emit(&mut program, Opcode::Xor32, vec![eax.clone(), eax], None);
    emit(&mut program, Opcode::Ret, vec![], None);
    let artifact = assemble_program(&program)?;
    let asm = render_asm(&program)?;
    let mut origins = String::from("# Section-relative byte ranges, NOT linked addresses\n");
    for entry in &artifact.entries {
        if let Some(range) = entry.range {
            origins.push_str(&format!("entry={} origins={:?} synthetic={} {:?} [{}..{})\n",
                entry.entry.0, entry.provenance.origins, entry.provenance.synthetic,
                range.section, range.start, range.end));
        }
    }
    Ok(Output { object: artifact.object, asm, origins, warnings: parsed.warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        let o = compile("出勤\n報告 \"こんにちは\"\n定時退社\n", Target::X86_64UnknownLinuxGnu).unwrap();
        assert!(o.object.starts_with(b"\x7fELF"));
        assert!(o.asm.contains("puts"));
        assert!(o.origins.contains("OriginId(2)"));
    }
    #[test]
    fn windows_object() {
        let o = compile("出勤\n報告 \"hello\"\n定時退社", Target::X86_64PcWindowsMsvc).unwrap();
        assert!(!o.object.is_empty());
        assert!(o.asm.contains("puts"));
    }
    #[test]
    fn overtime_warning() {
        let o = compile("出勤\n残業 {\n有給\n}\n定時退社", Target::X86_64UnknownLinuxGnu).unwrap();
        assert_eq!(o.warnings.len(), 1);
        assert!(o.asm.contains("jmp"));
    }
    #[test]
    fn parser_errors() {
        assert!(parse("定時退社").is_err());
        assert!(parse("出勤\n残業 {").is_err());
        assert!(parse("出勤\n}").is_err());
        assert!(parse("出勤\n報告 \"abc\\q\"").is_err());
    }
}
