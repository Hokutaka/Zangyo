//! Zangyo: a tiny, deliberately over-serious joke compiler.
use sashimi_assembler::{
    Target,
    program::{assemble_program, render_asm, EntryKind, Linkage, OriginId, Program, Provenance, Section},
    typed::{Instruction, Opcode, Operand, Register, Width},
};
use std::{collections::{BTreeMap, BTreeSet}, error::Error, fmt};
mod expr;
use expr::{Expr, Operator, parse_expression, valid_name};

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
    ReportNumber { expr: Expr, line: usize },
    Assign { name: String, expr: Expr, line: usize },
    Overtime { body: Vec<Statement>, line: usize },
    Leave { line: usize },
    Vacation { line: usize },
    Collapse { line: usize },
}
impl Statement {
    fn line(&self) -> usize {
        match self {
            Self::Report { line, .. } | Self::ReportNumber { line, .. } | Self::Assign { line, .. } | Self::Overtime { line, .. }
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
            return Err(Issue { line, message: "文字列内の引用符はエスケープしてください".into() });
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
            _ if text.starts_with("報告 ") => {
                let argument = text.strip_prefix("報告 ").unwrap().trim();
                if argument.starts_with('"') {
                    Statement::Report { text: parse_string(argument, line)?, line }
                } else {
                    Statement::ReportNumber { expr: parse_expression(argument, line)?, line }
                }
            },
            _ if text.contains('=') => {
                let (name, expr) = text.split_once('=').unwrap();
                let name = name.trim();
                if !valid_name(name) {
                    return Err(Issue { line, message: format!("不正な変数名です: {name}") });
                }
                Statement::Assign { name: name.into(), expr: parse_expression(expr.trim(), line)?, line }
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

fn reg(n: u8) -> Result<Operand, Box<dyn Error>> {
    Ok(Register::gpr(n, Width::Qword)?.into())
}
fn stack(offset: i32) -> Result<Operand, Box<dyn Error>> {
    Ok(sashimi_assembler::typed::Memory::new(Register::gpr(5, Width::Qword)?, None, -offset)?.into())
}
fn op(p: &mut Program, code: Opcode, operands: Vec<Operand>, line: usize) {
    emit(p, code, operands, Some(line));
}
fn check_expr(expr: &Expr, initialized: &BTreeSet<String>, line: usize) -> Result<(), Issue> {
    match expr {
        Expr::Integer(_) => Ok(()),
        Expr::Variable(name) => {
            if initialized.contains(name) { Ok(()) } else {
                Err(Issue { line, message: format!("未定義または未初期化の変数です: {name}") })
            }
        }
        Expr::Neg(inner) => check_expr(inner, initialized, line),
        Expr::Binary(left, _, right) => {
            check_expr(left, initialized, line)?;
            check_expr(right, initialized, line)
        }
    }
}
fn inspect(stmts: &[Statement], initialized: &mut BTreeSet<String>, slots: &mut BTreeMap<String, i32>) -> Result<(), Issue> {
    for stmt in stmts {
        match stmt {
            Statement::Assign { name, expr, line } => {
                check_expr(expr, initialized, *line)?;
                if !slots.contains_key(name) {
                    let offset = i32::try_from((slots.len() + 1) * 8).map_err(|_| Issue {
                        line: *line, message: "変数が多すぎます".into()
                    })?;
                    slots.insert(name.clone(), offset);
                }
                initialized.insert(name.clone());
            }
            Statement::ReportNumber { expr, line } => check_expr(expr, initialized, *line)?,
            Statement::Overtime { body, .. } => inspect(body, initialized, slots)?,
            _ => {}
        }
    }
    Ok(())
}
// Expression result: RAX. Temporary values live on the stack.
// The prologue aligns RSP, so each temporary push toggles stack alignment.
fn expression(expr: &Expr, p: &mut Program, slots: &BTreeMap<String, i32>, line: usize) -> Result<(), Box<dyn Error>> {
    match expr {
        Expr::Integer(n) => op(p, Opcode::MovAbs64, vec![Operand::Immediate(*n as i128), reg(0)?], line),
        Expr::Variable(name) => {
            let offset = *slots.get(name).expect("checked variable");
            op(p, Opcode::Mov64, vec![stack(offset)?, reg(0)?], line);
        }
        Expr::Neg(inner) => {
            expression(inner, p, slots, line)?;
            op(p, Opcode::Neg64, vec![reg(0)?], line);
        }
        Expr::Binary(left, operator, right) => {
            expression(left, p, slots, line)?;
            op(p, Opcode::Push, vec![reg(0)?], line);
            expression(right, p, slots, line)?;
            // RCX = right, RAX = left, RDX used by signed division.
            op(p, Opcode::Mov64, vec![reg(0)?, reg(1)?], line);
            op(p, Opcode::Pop, vec![reg(0)?], line);
            match operator {
                Operator::Add => op(p, Opcode::Add64, vec![reg(1)?, reg(0)?], line),
                Operator::Sub => op(p, Opcode::Sub64, vec![reg(1)?, reg(0)?], line),
                Operator::Mul => op(p, Opcode::SignedMul64, vec![reg(1)?, reg(0)?], line),
                Operator::Div => {
                    op(p, Opcode::SignExtendRax, vec![], line);
                    op(p, Opcode::SignedDiv64, vec![reg(1)?], line);
                }
            }
        }
    }
    Ok(())
}
fn frame_exit(p: &mut Program, line: Option<usize>) -> Result<(), Box<dyn Error>> {
    emit(p, Opcode::Xor32, vec![Register::gpr(0, Width::Dword)?.into(), Register::gpr(0, Width::Dword)?.into()], line);
    emit(p, Opcode::Mov64, vec![reg(5)?, reg(4)?], line);
    emit(p, Opcode::Pop, vec![reg(5)?], line);
    emit(p, Opcode::Ret, vec![], line);
    Ok(())
}
fn lower(
    stmts: &[Statement], p: &mut Program, target: Target,
    report_symbols: &mut impl Iterator<Item = sashimi_assembler::program::SymbolId>,
    puts: sashimi_assembler::program::SymbolId,
    printf: sashimi_assembler::program::SymbolId,
    integer_format: sashimi_assembler::program::SymbolId,
    slots: &BTreeMap<String, i32>,
) -> Result<(), Box<dyn Error>> {
    let windows = target == Target::X86_64PcWindowsMsvc;
    let arg = reg(if windows { 1 } else { 7 })?;
    let rsp = reg(4)?;
    let shadow = if windows { 32 } else { 0 };
    for stmt in stmts {
        match stmt {
            Statement::Report { line, .. } => {
                let sym = report_symbols.next().expect("report string symbol");
                op(p, Opcode::Lea64, vec![Operand::RipRelative(sym), arg.clone()], *line);
                if shadow > 0 { op(p, Opcode::Sub64, vec![Operand::Immediate(shadow), rsp.clone()], *line); }
                op(p, Opcode::Call, vec![Operand::Symbol(puts)], *line);
                if shadow > 0 { op(p, Opcode::Add64, vec![Operand::Immediate(shadow), rsp.clone()], *line); }
            }
            Statement::ReportNumber { expr, line } => {
                expression(expr, p, slots, *line)?;
                // printf("%lld\n", value): Windows RCX/RDX; SysV RDI/RSI.
                op(p, Opcode::Mov64, vec![reg(0)?, reg(if windows { 2 } else { 6 })?], *line);
                op(p, Opcode::Lea64, vec![Operand::RipRelative(integer_format), arg.clone()], *line);
                // SysV variadic ABI: AL is the number of vector registers used.
                if !windows { op(p, Opcode::Xor32, vec![Register::gpr(0, Width::Dword)?.into(), Register::gpr(0, Width::Dword)?.into()], *line); }
                if shadow > 0 { op(p, Opcode::Sub64, vec![Operand::Immediate(shadow), rsp.clone()], *line); }
                op(p, Opcode::Call, vec![Operand::Symbol(printf)], *line);
                if shadow > 0 { op(p, Opcode::Add64, vec![Operand::Immediate(shadow), rsp.clone()], *line); }
            }
            Statement::Assign { name, expr, line } => {
                expression(expr, p, slots, *line)?;
                op(p, Opcode::Mov64, vec![reg(0)?, stack(slots[name])?], *line);
            }
            Statement::Leave { line } => frame_exit(p, Some(*line))?,
            Statement::Vacation { line } => {
                p.push(EntryKind::Marker, Provenance::source(OriginId(*line as u64)));
            }
            Statement::Collapse { line } => op(p, Opcode::Undefined, vec![], *line),
            Statement::Overtime { body, line } => {
                let label = p.declare(format!(".Lzangyo_{}", p.symbols.len()), Linkage::Local);
                p.push(EntryKind::Label(label), Provenance::source(OriginId(*line as u64)));
                lower(body, p, target, report_symbols, puts, printf, integer_format, slots)?;
                op(p, Opcode::Jump, vec![Operand::Symbol(label)], *line);
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
    let mut slots = BTreeMap::new();
    inspect(&parsed.statements, &mut BTreeSet::new(), &mut slots)?;
    let mut program = Program::new(target);
    let main = program.declare("main", Linkage::Global);
    let mut strings = vec![];
    gather(&parsed.statements, &mut strings);
    let mut reports = vec![];
    for (i, _) in strings.iter().enumerate() {
        reports.push(program.declare(format!(".Lreport_{i}"), Linkage::Local));
    }
    let puts = program.declare("puts", Linkage::External);
    let printf = program.declare("printf", Linkage::External);
    let integer_format = program.declare(".Linteger_format", Linkage::Local);
    {
        program.push(EntryKind::Section(Section::ReadOnlyData), Provenance::synthetic());
        // Always define the local format symbol, even for string-only programs.
        program.push(EntryKind::Label(integer_format), Provenance::synthetic());
        program.push(EntryKind::CString("%lld\n".into()), Provenance::synthetic());
        for ((line, value), symbol) in strings.iter().zip(reports.iter()) {
            program.push(EntryKind::Label(*symbol), Provenance::source(OriginId(*line as u64)));
            program.push(EntryKind::CString(value.clone()), Provenance::source(OriginId(*line as u64)));
        }
    }
    program.push(EntryKind::Section(Section::Text), Provenance::synthetic());
    program.push(EntryKind::Label(main), Provenance::synthetic());
    // Preserve RBP and align RSP to 16 bytes before calling external functions.
    emit(&mut program, Opcode::Push, vec![reg(5)?], None);
    emit(&mut program, Opcode::Mov64, vec![reg(4)?, reg(5)?], None);
    let frame_size = (i64::from(slots.len() as i32) * 8 + 15) & !15;
    if frame_size > 0 { emit(&mut program, Opcode::Sub64, vec![Operand::Immediate(frame_size as i128), reg(4)?], None); }
    let mut report_symbols = reports.into_iter();
    lower(&parsed.statements, &mut program, target, &mut report_symbols, puts, printf, integer_format, &slots)?;
    frame_exit(&mut program, None)?;
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
    #[test]
    fn arithmetic_codegen() {
        let source = "出勤\n基本給 = 250000\n残業時間 = 40\n残業代 = 残業時間 * 1500\n給料 = 基本給 + 残業代\n報告 給料\n定時退社";
        for target in [Target::X86_64UnknownLinuxGnu, Target::X86_64PcWindowsMsvc] {
            let o = compile(source, target).unwrap();
            assert!(o.asm.contains("imulq"));
            assert!(o.asm.contains("printf"));
            assert!(o.origins.contains("OriginId(6)"));
        }
    }
    #[test]
    fn expression_errors() {
        assert!(parse("出勤\n報告 1 +").is_err());
        assert!(parse("出勤\nA = 2 * (3").is_err());
        assert!(compile("出勤\n報告 未定義\n定時退社", Target::X86_64UnknownLinuxGnu).is_err());
        assert!(compile("出勤\nA = A + 1", Target::X86_64UnknownLinuxGnu).is_err());
    }

}
