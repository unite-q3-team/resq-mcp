//! Analysis session: one open QVM plus everything the tools need.

use qvm::{trap_name, Disassembly, Insn, Opcode, Qvm};
use std::collections::HashMap;

/// Per-function summary built once at open time.
pub struct FnRow {
    pub idx: usize,
    pub entry: usize,
    pub end: usize,
    pub name: Option<String>,
    /// (syscall number, resolved name or "?").
    pub traps: Vec<(u32, String)>,
    /// Distinct string literals referenced by this function.
    pub strings: Vec<String>,
    /// Direct CONST+CALL callees (function indices).
    pub callees: Vec<usize>,
}

/// One open QVM with precomputed tables.
pub struct Session {
    pub qvm: Qvm,
    pub d: Disassembly,
    pub fns: Vec<FnRow>,
    /// Literal-segment string table in address order (addr, value).
    pub lit_strings: Vec<(i32, String)>,
    /// name -> fn idx (map names, `fn_<idx>` placeholders, trap names of
    /// thunks are NOT unique - first wins).
    pub by_name: HashMap<String, usize>,
    /// fn idx -> caller fn indices (direct CONST+CALL).
    pub callers: HashMap<usize, Vec<usize>>,
}

impl Session {
    /// Load and precompute. `map_path` optionally adds/overrides function
    /// names from a q3asm `.map` file.
    pub fn open(path: &str, map_path: Option<&str>) -> Result<Session, String> {
        let mut qvm = qvm::load(path).map_err(|e| format!("load: {e}"))?;
        match map_path {
            Some(mp) => {
                let syms = qvm::load_map(mp).map_err(|e| format!("load {mp}: {e}"))?;
                for (entry, name) in syms {
                    qvm.names.insert(entry, name);
                }
            }
            None => {
                // Auto: sibling `.map` next to the QVM, when present.
                let sibling = std::path::Path::new(path).with_extension("map");
                if sibling.is_file() {
                    if let Ok(syms) = qvm::load_map(sibling.to_str().unwrap_or_default()) {
                        for (entry, name) in syms {
                            qvm.names.insert(entry, name);
                        }
                    }
                }
            }
        }
        let d = qvm::disassemble(&qvm).map_err(|e| format!("disasm: {e}"))?;
        let ranges = qvm::build_functions(&d);

        let mut fns: Vec<FnRow> = Vec::with_capacity(ranges.len());
        for (idx, &(start, end)) in ranges.iter().enumerate() {
            let mut traps: Vec<(u32, String)> = Vec::new();
            let mut strings: Vec<String> = Vec::new();
            let mut callees: Vec<usize> = Vec::new();
            for (k, ins) in d.insns[start..end].iter().enumerate() {
                let Some(opd) = ins.operand else { continue };
                if ins.op != Opcode::Const {
                    continue;
                }
                if let Some(s) = qvm.string_at(opd) {
                    if !strings.contains(&s) {
                        strings.push(s);
                    }
                }
                if let Some(next) = d.insns[start..end].get(k + 1) {
                    if next.op != Opcode::Call {
                        continue;
                    }
                    if opd < 0 {
                        let num = (-1 - opd) as u32;
                        let n = trap_name(qvm.module, num).unwrap_or("?").to_string();
                        if !traps.contains(&(num, n.clone())) {
                            traps.push((num, n));
                        }
                    } else if let Some(ci) = entry_index(&ranges, opd as usize) {
                        if !callees.contains(&ci) {
                            callees.push(ci);
                        }
                    }
                }
            }
            let name = qvm.name_for_fn(start).map(str::to_string);
            fns.push(FnRow {
                idx,
                entry: start,
                end,
                name,
                traps,
                strings,
                callees,
            });
        }

        // Literal-segment string table (address order).
        let mut lit_strings = Vec::new();
        let (mut a, top) = (qvm.data_length, qvm.data_length + qvm.lit_length);
        while a < top {
            match qvm.string_at(a) {
                Some(s) => {
                    let step = s.len() as i32 + 1;
                    lit_strings.push((a, s));
                    a += step;
                }
                None => a += 1,
            }
        }

        let mut by_name: HashMap<String, usize> = HashMap::new();
        for f in &fns {
            if let Some(n) = &f.name {
                by_name.entry(n.clone()).or_insert(f.idx);
            }
        }
        // Placeholders so `fn_12` resolves like in the GUI.
        for f in &fns {
            by_name.entry(format!("fn_{}", f.idx)).or_insert(f.idx);
        }

        let mut callers: HashMap<usize, Vec<usize>> = HashMap::new();
        for f in &fns {
            for &c in &f.callees {
                callers.entry(c).or_default().push(f.idx);
            }
        }

        Ok(Session {
            qvm,
            d,
            fns,
            lit_strings,
            by_name,
            callers,
        })
    }

    /// Resolve a tool's `fn` argument: integer = index, string = name
    /// (map name or `fn_<idx>` placeholder).
    pub fn resolve_fn(&self, spec: &serde_json::Value) -> Result<usize, String> {
        if let Some(i) = spec.as_u64() {
            let idx = i as usize;
            return self
                .fns
                .get(idx)
                .map(|_| idx)
                .ok_or_else(|| format!("fn index {idx} out of range (0..{})", self.fns.len()));
        }
        let name = spec
            .as_str()
            .ok_or_else(|| "fn must be an integer index or a string name".to_string())?;
        self.by_name
            .get(name)
            .copied()
            .ok_or_else(|| format!("no function named {name:?}"))
    }

    /// Identity C for a function.
    pub fn decompile(&self, idx: usize) -> Result<String, String> {
        let f = self.fns.get(idx).ok_or("bad fn index")?;
        let data = self.qvm.data_int32();
        let cfg = qvm::build_cfg(&self.d, (f.entry, f.end), &data)
            .ok_or_else(|| "degenerate CFG".to_string())?;
        let frame = self.d.insns[cfg.entry].operand.unwrap_or(0);
        let fun = qvm::decompile_function(&self.d, &cfg, frame, &data);
        Ok(qvm::fmt_function(&fun, &self.qvm))
    }

    /// Plain disassembly text for a function.
    pub fn disasm_text(&self, idx: usize) -> String {
        let f = &self.fns[idx];
        let mut out = String::new();
        for ins in &self.d.insns[f.entry..f.end] {
            out.push_str(&format_insn(ins, self));
            out.push('\n');
        }
        out
    }

    /// Functions referencing `addr` via CONST, with hit counts.
    pub fn xrefs_to(&self, addr: i32) -> Vec<(usize, u32)> {
        let mut out: Vec<(usize, u32)> = Vec::new();
        for f in &self.fns {
            let mut n = 0u32;
            for ins in &self.d.insns[f.entry..f.end] {
                if ins.op == Opcode::Const && ins.operand == Some(addr) {
                    n += 1;
                }
            }
            if n > 0 {
                out.push((f.idx, n));
            }
        }
        out
    }

    /// One-line classification of a memory address (data/lit/BSS/string/ptr).
    pub fn mem_hint(&self, addr: i32) -> Option<String> {
        if addr == 0 {
            return Some("NULL".into());
        }
        let (dl, ll, bl) = (
            self.qvm.data_length,
            self.qvm.lit_length,
            self.qvm.bss_length,
        );
        let (lit0, bss0) = (dl, dl + ll);
        let (bss1, _) = (bss0 + bl, i32::MAX);
        let a = addr;
        if a < 0 || a >= bss1 {
            return None; // call target or garbage
        }
        if a < lit0 {
            let mut h = format!("[{a:#x}] data");
            if let Some(s) = self.qvm.string_at(a) {
                h.push_str(&format!(" = \"{}\"", crate::tools::clip(&s, 60)));
            } else {
                let data = self.qvm.data_int32();
                if let Some(v) = data.get(a as usize) {
                    h.push_str(&format!(" = {v} ({v:#x})"));
                }
            }
            return Some(h);
        }
        if a < lit0 {
            let mut h = format!("[{a:#x}] lit");
            let off = (a - lit0) as usize;
            if off + 4 <= self.qvm.lit.len() {
                let l = &self.qvm.lit;
                let f = f32::from_le_bytes([l[off], l[off + 1], l[off + 2], l[off + 3]]);
                h.push_str(&format!(" = float {f}"));
            }
            if let Some(s) = self.qvm.string_at(a) {
                h.push_str(&format!("; string \"{}\"", crate::tools::clip(&s, 60)));
            }
            return Some(h);
        }
        Some(format!("[{a:#x}] BSS = runtime global (zero at load)"))
    }
}

fn entry_index(ranges: &[(usize, usize)], insn: usize) -> Option<usize> {
    // Linear probe is fine: ranges are sorted; swap to binary search later.
    ranges.iter().position(|&(s, _)| s == insn)
}

/// One disasm line, matching the GUI's enriched shape.
fn format_insn(ins: &Insn, s: &Session) -> String {
    let mut line = format!("{ins}");
    if ins.op != Opcode::Const {
        return line;
    }
    let Some(opd) = ins.operand else { return line };
    if let Some(str_val) = s.qvm.string_at(opd) {
        line.push_str(&format!("  ; \"{}\"", crate::tools::clip(&str_val, 60)));
    }
    if let Some(next) = s.d.insns.get(ins.idx + 1) {
        if next.op == Opcode::Call && opd < 0 {
            let num = (-1 - opd) as u32;
            match trap_name(s.qvm.module, num) {
                Some(n) => line.push_str(&format!("  ; syscall {num} {n}")),
                None => line.push_str(&format!("  ; syscall {num}")),
            }
        } else if next.op == Opcode::Call && opd >= 0 {
            match s.qvm.name_for_fn(opd as usize) {
                Some(n) => line.push_str(&format!("  ; call {n}")),
                None => line.push_str(&format!("  ; call fn@{opd:x}")),
            }
        }
    }
    line
}
