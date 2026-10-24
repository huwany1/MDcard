// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Source-preserving structural editing. AST offsets are bytes; UI cursors are characters.
//! We never serialize the AST: unaffected source stays byte-for-byte identical.
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Block {
    pub range: Range<usize>,
    pub parent: Option<usize>,
    pub kind: &'static str,
    pub indent: usize,
    pub group: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub range: Range<usize>,
    pub cells: Vec<Range<usize>>,
    pub ending: String,
}

#[derive(Clone, Debug)]
pub struct Table {
    /// Header, delimiter, then body rows, including extra/missing cells.
    pub rows: Vec<Row>,
    pub columns: usize,
}

#[derive(Default)]
pub struct Index {
    pub blocks: Vec<Block>,
    pub tables: Vec<Table>,
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |n| n + 1)
}

fn line_end(text: &str, at: usize) -> usize {
    if at == 0 || text.as_bytes().get(at - 1) != Some(&b'\n') {
        text[at..].find('\n').map_or(text.len(), |n| at + n + 1)
    } else {
        at
    }
}

impl Index {
    pub fn parse(text: &str) -> Self {
        let mut index = Self::default();
        let mut depth = 0usize;
        let mut items: Vec<usize> = Vec::new();
        let mut lists: Vec<usize> = Vec::new();
        let mut top = None;
        for (event, range) in Parser::new_ext(text, Options::all()).into_offset_iter() {
            match event {
                Event::Start(tag) => {
                    if matches!(tag, Tag::List(_)) {
                        lists.push(range.start);
                    }
                    let kind = match tag {
                        Tag::Paragraph => "Paragraph",
                        Tag::Heading { .. } => "Heading",
                        Tag::Table(_) => "Table",
                        Tag::List(_) => "List",
                        Tag::Item => "List item",
                        Tag::BlockQuote(_) => "Quote",
                        Tag::CodeBlock(_) => "Code",
                        Tag::HtmlBlock => "HTML",
                        _ => "Block",
                    };
                    if depth == 0 || matches!(tag, Tag::Item) {
                        let start = line_start(text, range.start);
                        let end = line_end(text, range.end);
                        let parent = if depth == 0 {
                            None
                        } else {
                            items.last().copied().or(top)
                        };
                        let node = index.blocks.len();
                        index.blocks.push(Block {
                            group: if matches!(tag, Tag::Item) {
                                lists.last().copied()
                            } else {
                                None
                            },
                            range: start..end,
                            parent,
                            kind,
                            indent: text[start..].bytes().take_while(|c| *c == b' ').count(),
                        });
                        if depth == 0 {
                            top = Some(node);
                        }
                        if matches!(tag, Tag::Item) {
                            items.push(node);
                        }
                    }
                    // Nested/quoted tables require container-prefix rewriting; keep those in source.
                    if depth == 0 && matches!(tag, Tag::Table(_)) {
                        let start = line_start(text, range.start);
                        let end = line_end(text, range.end);
                        let mut offset = start;
                        let rows: Vec<Row> = text[start..end]
                            .split_inclusive('\n')
                            .map(|line| {
                                let row = parse_row(line, offset);
                                offset += line.len();
                                row
                            })
                            .collect();
                        if rows.len() >= 2 && !rows[0].cells.is_empty() {
                            let columns = rows[0].cells.len();
                            index.tables.push(Table { rows, columns });
                        }
                    }
                    depth += 1;
                }
                Event::End(end) => {
                    if matches!(end, pulldown_cmark::TagEnd::List(_)) {
                        lists.pop();
                    }
                    if matches!(end, pulldown_cmark::TagEnd::Item) {
                        items.pop();
                    }
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        top = None;
                    }
                }
                Event::Rule if depth == 0 => index.blocks.push(Block {
                    group: None,
                    range: line_start(text, range.start)..line_end(text, range.end),
                    parent: None,
                    kind: "Rule",
                    indent: 0,
                }),
                _ => {}
            }
        }
        index
    }

    pub fn block_at(&self, byte: usize) -> Option<usize> {
        self.blocks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, b)| {
                b.range.contains(&byte)
                    || (byte == b.range.end
                        && byte == self.blocks.last().map_or(0, |b| b.range.end))
            })
            .map(|(i, _)| i)
    }
    pub fn table_at(&self, byte: usize) -> Option<(usize, usize, usize)> {
        self.tables.iter().enumerate().find_map(|(t, table)| {
            table.rows.iter().enumerate().find_map(|(r, row)| {
                if row.range.contains(&byte) {
                    let c = row
                        .cells
                        .iter()
                        .position(|cell| byte <= cell.end)
                        .unwrap_or(row.cells.len().saturating_sub(1));
                    Some((t, r, c))
                } else {
                    None
                }
            })
        })
    }
}

fn parse_row(line: &str, offset: usize) -> Row {
    let content = line.trim_end_matches(['\r', '\n']);
    let bytes = content.as_bytes();
    let mut pipes = Vec::new();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'|' {
            // GFM firstpass treats any immediately preceding slash as escaped,
            // even when it belongs to an even-length slash run.
            if i == 0 || bytes[i - 1] != b'\\' {
                pipes.push(i);
            }
        }
    }
    let first = content.len() - content.trim_start().len();
    let last = content.trim_end().len();
    let start = if pipes.first() == Some(&first) {
        pipes.remove(0) + 1
    } else {
        0
    };
    let end = if pipes.last().is_some_and(|p| *p + 1 == last) {
        pipes.pop().unwrap()
    } else {
        content.len()
    };
    let mut boundaries = vec![start];
    boundaries.extend(pipes.iter().map(|p| p + 1));
    let mut ends = pipes;
    ends.push(end);
    let cells = boundaries
        .into_iter()
        .zip(ends)
        .map(|(a, b)| offset + a..offset + b)
        .collect();
    Row {
        range: offset..offset + line.len(),
        cells,
        ending: line[content.len()..].into(),
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    DeleteTable {
        table: usize,
    },
    Cell {
        table: usize,
        row: usize,
        column: usize,
        value: String,
    },
    AddRow {
        table: usize,
        after: usize,
    },
    DeleteRow {
        table: usize,
        row: usize,
    },
    AddColumn {
        table: usize,
        column: usize,
    },
    DeleteColumn {
        table: usize,
        column: usize,
    },
    Align {
        table: usize,
        column: usize,
        alignment: usize,
    },
    Move {
        block: usize,
        target: usize,
    },
    Indent {
        block: usize,
        outdent: bool,
    },
    InsertTable {
        at: usize,
    },
}

#[derive(Clone, Debug)]
struct Patch {
    range: Range<usize>,
    value: String,
}

pub struct Change {
    pub text: String,
    pub selection: [usize; 2],
    pub block_anchor: Option<(usize, &'static str)>,
}

pub fn byte_at(text: &str, character: usize) -> usize {
    text.char_indices()
        .nth(character)
        .map_or(text.len(), |(i, _)| i)
}
pub fn char_at(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

impl Index {
    pub fn apply(
        &self,
        text: &str,
        selection: [usize; 2],
        command: Command,
    ) -> Result<Change, &'static str> {
        let mut patches = Vec::new();
        let mut mapped = selection.map(|c| byte_at(text, c));
        let bytes = mapped[0].min(mapped[1])..mapped[0].max(mapped[1]);
        // A structural command acts on one block only; never silently eat a cross-block selection.
        if !bytes.is_empty() {
            let a = self.block_at(bytes.start);
            let b = self.block_at(bytes.end.saturating_sub(1));
            if a != b {
                return Err("Select within one block first.");
            }
        }
        match command {
            Command::Move { block, target } => {
                let a = self.blocks.get(block).ok_or("Choose a block first.")?;
                let b = self.blocks.get(target).ok_or("Choose a block first.")?;
                if block == target {
                    return Err("Choose another block.");
                }
                if a.parent != b.parent || a.indent != b.indent || a.group != b.group {
                    return Err("Move between siblings at the same level.");
                }
                if matches!(a.kind, "Code" | "HTML") || matches!(b.kind, "Code" | "HTML") {
                    return Err("Protected code or HTML block.");
                }
                let down = a.range.start < b.range.start;
                let (left, right) = if down {
                    (&a.range, &b.range)
                } else {
                    (&b.range, &a.range)
                };
                if left.end > right.start {
                    return Err("Move between siblings at the same level.");
                }
                let siblings: Vec<_> = self
                    .blocks
                    .iter()
                    .filter(|n| n.parent == a.parent && n.indent == a.indent && n.group == a.group)
                    .collect();
                let (middle, gap) = if down {
                    let next = siblings
                        .iter()
                        .find(|n| n.range.start > a.range.start)
                        .unwrap();
                    (next.range.start..b.range.end, a.range.end..next.range.start)
                } else {
                    let previous = siblings
                        .iter()
                        .rev()
                        .find(|n| n.range.start < a.range.start)
                        .unwrap();
                    (
                        b.range.start..previous.range.end,
                        previous.range.end..a.range.start,
                    )
                };
                let (body, ending) = split_ending(&text[a.range.clone()]);
                let (other, other_ending) = split_ending(&text[middle.clone()]);
                let separator = &text[gap.clone()];
                let (value, destination, middle_destination) = if down {
                    (
                        format!("{other}{ending}{separator}{body}{other_ending}"),
                        left.start + other.len() + ending.len() + separator.len(),
                        left.start,
                    )
                } else {
                    (
                        format!("{body}{other_ending}{separator}{other}{ending}"),
                        left.start,
                        left.start + body.len() + other_ending.len() + separator.len(),
                    )
                };
                let selected_source = a.range.contains(&bytes.start) && bytes.end <= a.range.end;
                for cursor in &mut mapped {
                    *cursor = if a.range.contains(cursor)
                        || (*cursor == a.range.end && (selected_source || *cursor == text.len()))
                    {
                        destination + (*cursor - a.range.start).min(body.len())
                    } else if middle.contains(cursor) {
                        middle_destination + (*cursor - middle.start).min(other.len())
                    } else if gap.contains(cursor) {
                        if down {
                            left.start + other.len() + ending.len() + (*cursor - gap.start)
                        } else {
                            left.start + body.len() + other_ending.len() + (*cursor - gap.start)
                        }
                    } else {
                        *cursor
                    };
                }
                patches.push(Patch {
                    range: left.start..right.end,
                    value,
                });
                let changed = apply_patches(text, patches, None);
                return Ok(Change {
                    block_anchor: Some((destination, a.kind)),
                    selection: mapped.map(|b| char_at(&changed.text, b.min(changed.text.len()))),
                    text: changed.text,
                });
            }
            Command::Indent { block, outdent } => {
                let node = self.blocks.get(block).ok_or("Choose a list item first.")?;
                if node.kind != "List item" {
                    return Err("Choose a list item first.");
                }
                let siblings: Vec<_> = self
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| {
                        n.parent == node.parent && n.indent == node.indent && n.group == node.group
                    })
                    .collect();
                let position = siblings.iter().position(|(id, _)| *id == block).unwrap();
                let width = if outdent {
                    let parent = node
                        .parent
                        .and_then(|p| self.blocks.get(p))
                        .filter(|p| p.kind == "List item")
                        .ok_or("This item is already at the outer level.")?;
                    if position + 1 != siblings.len()
                        || self
                            .blocks
                            .iter()
                            .any(|n| n.parent == node.parent && n.range.start > node.range.start)
                    {
                        return Err("Outdent the last sibling first.");
                    }
                    node.indent.saturating_sub(parent.indent)
                } else {
                    if position == 0 {
                        return Err("The first item cannot be indented.");
                    }
                    let previous = siblings[position - 1].1;
                    let line = &text[previous.range.start..previous.range.end];
                    marker_width(line).ok_or("Complex list prefix: edit in source.")?
                };
                if width == 0 || width > 12 || node.indent + width > 48 {
                    return Err("Complex list prefix: edit in source.");
                }
                let mut at = node.range.start;
                for line in text[node.range.clone()].split_inclusive('\n') {
                    if !line.trim().is_empty() {
                        if outdent && !line.starts_with(&" ".repeat(width)) {
                            return Err("Complex list prefix: edit in source.");
                        }
                        patches.push(Patch {
                            range: at..at + if outdent { width } else { 0 },
                            value: if outdent {
                                String::new()
                            } else {
                                " ".repeat(width)
                            },
                        });
                    }
                    at += line.len();
                }
            }
            Command::InsertTable { at } => {
                if self
                    .blocks
                    .iter()
                    .any(|b| b.range.start < at && at < b.range.end)
                {
                    return Err("Insert a table between blocks.");
                }
                let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
                patches.push(Patch { range: at..at, value: format!("{newline}| A | B |{newline}| --- | --- |{newline}|   |   |{newline}{newline}") });
            }
            command => {
                let table_id = match &command {
                    Command::DeleteTable { table }
                    | Command::Cell { table, .. }
                    | Command::AddRow { table, .. }
                    | Command::DeleteRow { table, .. }
                    | Command::AddColumn { table, .. }
                    | Command::DeleteColumn { table, .. }
                    | Command::Align { table, .. } => *table,
                    _ => unreachable!(),
                };
                let table = self.tables.get(table_id).ok_or("Choose a table first.")?;
                match command {
                    Command::DeleteTable { .. } => patches.push(Patch {
                        range: table.rows[0].range.start..table.rows.last().unwrap().range.end,
                        value: String::new(),
                    }),
                    Command::Cell {
                        row, column, value, ..
                    } => {
                        if row == 0 && column >= table.columns {
                            return Err("Choose a column first.");
                        }
                        let row = table
                            .rows
                            .get(row)
                            .filter(|_| row != 1)
                            .ok_or("Choose a body or header cell.")?;
                        let value = escape_cell(&value);
                        if let Some(cell) = row.cells.get(column) {
                            let raw = &text[cell.clone()];
                            let leading = raw.len() - raw.trim_start().len();
                            let trailing = raw.len() - raw.trim_end().len();
                            let end = cell.end.saturating_sub(trailing).max(cell.start + leading);
                            let range = cell.start + leading..end;
                            let mut value = value;
                            {
                                // Explicit edge borders keep both empty cells and block
                                // prefixes (#, -, fences, HTML, references) inside the row.
                                if column == 0 && row.cells[0].start == row.range.start {
                                    if range.start == row.range.start {
                                        value.insert(0, '|');
                                    } else {
                                        patches.push(Patch {
                                            range: row.range.start..row.range.start,
                                            value: "|".into(),
                                        });
                                    }
                                }
                                let content_end = row.range.end - row.ending.len();
                                if column + 1 == row.cells.len() && cell.end == content_end {
                                    if range.end == content_end {
                                        value.push('|');
                                    } else {
                                        patches.push(Patch {
                                            range: content_end..content_end,
                                            value: "|".into(),
                                        });
                                    }
                                }
                            }
                            patches.push(Patch { range, value });
                        } else {
                            let at = row.cells.last().unwrap().end;
                            patches.push(Patch {
                                range: at..at,
                                value: format!(
                                    "{} {value} ",
                                    " | ".repeat(column + 1 - row.cells.len())
                                ),
                            });
                        }
                    }
                    Command::AddRow { after, .. } => {
                        let row = table.rows.get(after).ok_or("Choose a row first.")?;
                        let row = if after < 2 { &table.rows[1] } else { row };
                        let newline = table
                            .rows
                            .iter()
                            .find(|r| !r.ending.is_empty())
                            .map_or("\n", |r| r.ending.as_str());
                        let prefix = if row.ending.is_empty() { newline } else { "" };
                        let suffix = if row.ending.is_empty() { "" } else { newline };
                        patches.push(Patch {
                            range: row.range.end..row.range.end,
                            value: format!(
                                "{prefix}|{}|{suffix}",
                                "   |".repeat(table.columns).trim_end_matches('|')
                            ),
                        });
                    }
                    Command::DeleteRow { row, .. } => {
                        if row < 2 {
                            return Err("The table header cannot be deleted.");
                        }
                        let current = table.rows.get(row).ok_or("Choose a row first.")?;
                        let mut range = current.range.clone();
                        if current.ending.is_empty() && row + 1 == table.rows.len() {
                            range.start -= table.rows[row - 1].ending.len();
                        }
                        patches.push(Patch {
                            range,
                            value: String::new(),
                        });
                    }
                    Command::AddColumn { column, .. } => {
                        if column > table.columns {
                            return Err("Choose a column first.");
                        }
                        for (r, row) in table.rows.iter().enumerate() {
                            let value = if r == 1 { " --- " } else { "   " };
                            let (at, mut value) = if let Some(cell) = row.cells.get(column) {
                                (cell.start, format!("{value}|"))
                            } else {
                                let at = row.cells.last().unwrap().end;
                                let padding = "|   ".repeat(column.saturating_sub(row.cells.len()));
                                (at, format!("{padding}|{value}"))
                            };
                            let start = row.range.start;
                            let end = row.range.end - row.ending.len();
                            if row.cells[0].start == start {
                                if at == start {
                                    value.insert(0, '|');
                                } else {
                                    patches.push(Patch {
                                        range: start..start,
                                        value: "|".into(),
                                    });
                                }
                            }
                            if row.cells.last().unwrap().end == end {
                                if at == end {
                                    value.push('|');
                                } else {
                                    patches.push(Patch {
                                        range: end..end,
                                        value: "|".into(),
                                    });
                                }
                            }
                            patches.push(Patch {
                                range: at..at,
                                value,
                            });
                        }
                    }
                    Command::DeleteColumn { column, .. } => {
                        if table.columns <= 1 {
                            return Err("Keep at least one table column.");
                        }
                        if column >= table.columns {
                            return Err("Choose a column first.");
                        }
                        for row in &table.rows {
                            let range = row.cells.get(column).map(|cell| {
                                if let Some(next) = row.cells.get(column + 1) {
                                    cell.start..next.start
                                } else if column > 0 {
                                    row.cells[column - 1].end..cell.end
                                } else {
                                    cell.clone()
                                }
                            });
                            let mut value = String::new();
                            // Explicit borders preserve empty/ragged rows and prevent
                            // surviving block prefixes from escaping the table.
                            {
                                let start = row.range.start;
                                let end = row.range.end - row.ending.len();
                                if row.cells[0].start == start {
                                    if range.as_ref().is_some_and(|r| r.start == start) {
                                        value.push('|');
                                    } else {
                                        patches.push(Patch {
                                            range: start..start,
                                            value: "|".into(),
                                        });
                                    }
                                }
                                if row.cells.last().unwrap().end == end {
                                    if range.as_ref().is_some_and(|r| r.end == end) {
                                        value.push('|');
                                    } else {
                                        patches.push(Patch {
                                            range: end..end,
                                            value: "|".into(),
                                        });
                                    }
                                }
                            }
                            if let Some(range) = range {
                                patches.push(Patch { range, value });
                            }
                        }
                    }
                    Command::Align {
                        column, alignment, ..
                    } => {
                        let cell = table.rows[1]
                            .cells
                            .get(column)
                            .ok_or("Choose a column first.")?;
                        let value = match alignment {
                            1 => " :---: ",
                            2 => " ---: ",
                            _ => " :--- ",
                        };
                        patches.push(Patch {
                            range: cell.clone(),
                            value: value.into(),
                        });
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(apply_patches(text, patches, Some(mapped)))
    }

    pub fn neighbor(&self, block: usize, down: bool) -> Option<usize> {
        let node = self.blocks.get(block)?;
        let siblings: Vec<_> = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.parent == node.parent && b.indent == node.indent && b.group == node.group
            })
            .map(|(i, _)| i)
            .collect();
        let pos = siblings.iter().position(|i| *i == block)?;
        siblings
            .get(if down { pos + 1 } else { pos.checked_sub(1)? })
            .copied()
    }
}

fn marker_width(line: &str) -> Option<usize> {
    let s = line.trim_start_matches(' ');
    let end = if s.starts_with(['-', '+', '*']) {
        1
    } else {
        let digits = s.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !matches!(s.as_bytes().get(digits), Some(b'.' | b')')) {
            return None;
        }
        digits + 1
    };
    let spaces = s[end..].bytes().take_while(|c| *c == b' ').count();
    if spaces == 0 || spaces > 4 {
        None
    } else {
        Some(end + spaces)
    }
}

fn split_ending(s: &str) -> (&str, &str) {
    let body = s.trim_end_matches(['\r', '\n']);
    (body, &s[body.len()..])
}

fn escape_cell(value: &str) -> String {
    let mut result = String::new();
    let mut slashes = 0;
    for c in value.chars() {
        if c == '|' && slashes == 0 {
            result.push('\\');
        }
        match c {
            '\n' | '\r' => result.push(' '),
            _ => result.push(c),
        }
        slashes = if c == '\\' { slashes + 1 } else { 0 };
    }
    // A compact row may put the next delimiter immediately after this content.
    // An unmatched final slash must never turn that delimiter into cell text.
    if slashes > 0 {
        result.push(' ');
    }
    result
}

fn apply_patches(text: &str, mut patches: Vec<Patch>, selection: Option<[usize; 2]>) -> Change {
    patches.sort_by_key(|p| p.range.start);
    let map = |byte: usize| {
        let mut shift = 0isize;
        for patch in &patches {
            if byte < patch.range.start {
                break;
            }
            if patch.range.is_empty() && byte == patch.range.start {
                shift += patch.value.len() as isize;
                continue;
            }
            if byte < patch.range.end {
                return (patch.range.start as isize + shift) as usize
                    + (byte - patch.range.start).min(patch.value.len());
            }
            shift += patch.value.len() as isize - patch.range.len() as isize;
        }
        (byte as isize + shift) as usize
    };
    let mapped = selection.map(|s| s.map(map));
    let mut result = text.to_owned();
    for patch in patches.into_iter().rev() {
        result.replace_range(patch.range, &patch.value);
    }
    let selection = mapped.unwrap_or([0; 2]).map(|mut b| {
        b = b.min(result.len());
        while !result.is_char_boundary(b) {
            b -= 1;
        }
        char_at(&result, b)
    });
    Change {
        block_anchor: None,
        text: result,
        selection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_offsets_and_cell_patch_preserve_surrounding_bytes() {
        let s = "前 🦀\r\n\r\n| 名 | value |\r\n| :--- | ---: |\r\n| a\\|b | `x\\|y` |\r\n| short |\r\n| 1 | 2 | extra |\r\n\r\nafter";
        let index = Index::parse(s);
        assert_eq!(index.tables.len(), 1);
        let table = &index.tables[0];
        assert_eq!(table.rows[2].cells.len(), 2);
        assert_eq!(table.rows[4].cells.len(), 3);
        let change = index
            .apply(
                s,
                [2, 1],
                Command::Cell {
                    table: 0,
                    row: 2,
                    column: 0,
                    value: "中|文🦀".into(),
                },
            )
            .unwrap();
        assert_eq!(change.text, s.replacen("a\\|b", "中\\|文🦀", 1));
        assert_eq!(change.selection, [2, 1]);
        assert_eq!(Index::parse(&change.text).tables[0].rows[2].cells.len(), 2);
    }
    #[test]
    fn columns_keep_ragged_excess_and_unaffected_spacing() {
        let s = "a | b\n--- | ---\nx\n1 | 2 | extra";
        let index = Index::parse(s);
        let c = index
            .apply(
                s,
                [0, 0],
                Command::AddColumn {
                    table: 0,
                    column: 1,
                },
            )
            .unwrap();
        let i = Index::parse(&c.text);
        assert_eq!(i.tables[0].columns, 3);
        assert!(c.text.ends_with("|1 |   | 2 | extra|"));
        let c = i
            .apply(
                &c.text,
                [0, 0],
                Command::DeleteColumn {
                    table: 0,
                    column: 1,
                },
            )
            .unwrap();
        let next = Index::parse(&c.text);
        let raw: Vec<_> = next.tables[0].rows[3]
            .cells
            .iter()
            .map(|r| &c.text[r.clone()])
            .collect();
        assert_eq!(raw, ["1 ", " 2 ", " extra"]);
        assert!(!c.text.ends_with('\n'));
    }
    #[test]
    fn ragged_column_insertion_preserves_schema_rows_and_raw_cells() {
        for columns in 2..=4 {
            for count in 1..=columns + 1 {
                for border in 0..4 {
                    let wrap = |s: String| {
                        format!(
                            "{}{}{}",
                            if border & 1 != 0 { "|" } else { "" },
                            s,
                            if border & 2 != 0 { "|" } else { "" }
                        )
                    };
                    let header = wrap(
                        (0..columns)
                            .map(|i| format!(" h{i} "))
                            .collect::<Vec<_>>()
                            .join("|"),
                    );
                    let delimiter = wrap(vec![" --- "; columns].join("|"));
                    let body = wrap(
                        (0..count)
                            .map(|i| format!(" v{i} "))
                            .collect::<Vec<_>>()
                            .join("|"),
                    );
                    let source = format!("{header}\r\n{delimiter}\r\n{body}");
                    let index = Index::parse(&source);
                    assert_eq!(index.tables.len(), 1, "{source:?}");
                    for column in 0..=columns {
                        let change = index
                            .apply(&source, [0, 0], Command::AddColumn { table: 0, column })
                            .unwrap();
                        let next = Index::parse(&change.text);
                        assert_eq!(next.tables.len(), 1, "{column}: {:?}", change.text);
                        assert_eq!(next.tables[0].columns, columns + 1, "{:?}", change.text);
                        assert_eq!(next.tables[0].rows.len(), 3);
                        let row = &next.tables[0].rows[2];
                        for old in 0..count {
                            let destination = old + usize::from(old >= column);
                            assert_eq!(
                                &change.text[row.cells[destination].clone()],
                                format!(" v{old} ")
                            );
                        }
                        assert!(!change.text.ends_with('\n'));
                    }
                }
            }
        }
    }
    #[test]
    fn rows_alignment_and_terminal_newline() {
        for ending in ["", "\n", "\r\n"] {
            let s = format!("| a |\r\n| --- |\r\n| b |{ending}");
            let i = Index::parse(&s);
            let c = i
                .apply(&s, [0, 0], Command::AddRow { table: 0, after: 2 })
                .unwrap();
            assert_eq!(c.text.ends_with('\n'), !ending.is_empty());
            assert_eq!(Index::parse(&c.text).tables[0].rows.len(), 4);
            assert!(
                i.apply(&s, [0, 0], Command::DeleteRow { table: 0, row: 0 })
                    .is_err()
            );
            assert!(
                i.apply(
                    &s,
                    [0, 0],
                    Command::DeleteColumn {
                        table: 0,
                        column: 0
                    }
                )
                .is_err()
            );
            let c = i
                .apply(
                    &s,
                    [0, 0],
                    Command::Align {
                        table: 0,
                        column: 0,
                        alignment: 1,
                    },
                )
                .unwrap();
            assert_eq!(c.text, s.replace(" --- ", " :---: "));
        }
    }
    #[test]
    fn block_move_preserves_unicode_selection_and_missing_newline() {
        let s = "甲🦀\r\n\r\nsecond";
        let i = Index::parse(s);
        let c = i
            .apply(
                s,
                [2, 1],
                Command::Move {
                    block: 0,
                    target: 1,
                },
            )
            .unwrap();
        assert_eq!(c.text, "second\r\n\r\n甲🦀");
        assert_eq!(c.selection, [12, 11]);
        assert!(
            i.apply(
                s,
                [0, 8],
                Command::Move {
                    block: 0,
                    target: 1
                }
            )
            .is_err()
        );
    }
    #[test]
    fn distant_move_is_insertion_in_both_directions_and_maps_intermediate_selection() {
        let s = "A\n\nB\n\nC\n\nD";
        let i = Index::parse(s);
        let moved = i
            .apply(
                s,
                [0, 0],
                Command::Move {
                    block: 0,
                    target: 2,
                },
            )
            .unwrap();
        assert_eq!(moved.text, "B\n\nC\n\nA\n\nD");
        assert_eq!(moved.selection, [6, 6]);
        let back = Index::parse(&moved.text)
            .apply(
                &moved.text,
                [6, 6],
                Command::Move {
                    block: 2,
                    target: 0,
                },
            )
            .unwrap();
        assert_eq!(back.text, s);
        let moved = i
            .apply(
                s,
                [3, 3],
                Command::Move {
                    block: 0,
                    target: 2,
                },
            )
            .unwrap();
        assert_eq!(moved.selection, [0, 0]);
        let moved = i
            .apply(
                s,
                [9, 9],
                Command::Move {
                    block: 3,
                    target: 0,
                },
            )
            .unwrap();
        assert_eq!(moved.text, "D\n\nA\n\nB\n\nC");
        assert_eq!(moved.selection, [0, 0]);
    }
    #[test]
    fn list_subtree_moves_and_controlled_indent() {
        let s = "- a\n  - child\n- b\n- c\n";
        let i = Index::parse(s);
        assert_eq!(i.blocks.len(), 5);
        let c = i
            .apply(
                s,
                [0, 0],
                Command::Move {
                    block: 1,
                    target: 3,
                },
            )
            .unwrap();
        assert_eq!(c.text, "- b\n- a\n  - child\n- c\n");
        assert!(
            i.apply(
                s,
                [0, 0],
                Command::Indent {
                    block: 1,
                    outdent: false
                }
            )
            .is_err()
        );
        let c = i
            .apply(
                s,
                [0, 0],
                Command::Indent {
                    block: 3,
                    outdent: false,
                },
            )
            .unwrap();
        assert_eq!(c.text, "- a\n  - child\n  - b\n- c\n");
        let ii = Index::parse(&c.text);
        let b = ii
            .blocks
            .iter()
            .position(|n| n.kind == "List item" && c.text[n.range.clone()].starts_with("  - b"))
            .unwrap();
        let back = ii
            .apply(
                &c.text,
                [0, 0],
                Command::Indent {
                    block: b,
                    outdent: true,
                },
            )
            .unwrap();
        assert_eq!(back.text, s);
    }
    #[test]
    fn tight_list_boundary_belongs_to_next_item_and_selected_block_remains_stable() {
        let s = "- a\n- b\n- c\n- d\n";
        let i = Index::parse(s);
        let c = i
            .apply(
                s,
                [4, 4],
                Command::Move {
                    block: 1,
                    target: 2,
                },
            )
            .unwrap();
        assert_eq!(c.text, "- b\n- a\n- c\n- d\n");
        assert_eq!(c.selection, [0, 0]);
        let (anchor, kind) = c.block_anchor.unwrap();
        let ii = Index::parse(&c.text);
        let a = ii
            .blocks
            .iter()
            .position(|b| b.range.start == anchor && b.kind == kind)
            .unwrap();
        let cc = ii
            .apply(
                &c.text,
                [0, 0],
                Command::Move {
                    block: a,
                    target: ii.neighbor(a, true).unwrap(),
                },
            )
            .unwrap();
        assert_eq!(cc.text, "- b\n- c\n- a\n- d\n");
    }
    #[test]
    fn nested_list_groups_and_outdent_preserve_parent_boundaries() {
        let s = "- parent\n  - first\n\n  1. different group\n- next\n";
        let i = Index::parse(s);
        let children: Vec<_> = i
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == "List item" && b.indent == 2)
            .map(|(n, _)| n)
            .collect();
        assert_eq!(children.len(), 2);
        assert!(
            i.apply(
                s,
                [0, 0],
                Command::Move {
                    block: children[0],
                    target: children[1]
                }
            )
            .is_err()
        );
        assert!(
            i.apply(
                s,
                [0, 0],
                Command::Indent {
                    block: children[0],
                    outdent: true
                }
            )
            .is_err()
        );
    }
    #[test]
    fn code_html_and_nested_tables_are_protected() {
        let s = "```\n| a | b |\n| --- | --- |\n```\n\npara\n\n<div>\ntext\n</div>\n";
        let i = Index::parse(s);
        assert!(i.tables.is_empty());
        assert!(
            i.apply(
                s,
                [0, 0],
                Command::Move {
                    block: 0,
                    target: 1
                }
            )
            .is_err()
        );
        assert!(i.apply(s, [0, 0], Command::InsertTable { at: 8 }).is_err());
    }
    #[test]
    fn compact_cell_terminal_backslash_never_escapes_neighbor_delimiter() {
        let s = "|a|b|\n|---|---|\n|x|y|\n";
        for row in [0, 2] {
            let c = Index::parse(s)
                .apply(
                    s,
                    [0, 0],
                    Command::Cell {
                        table: 0,
                        row,
                        column: 0,
                        value: "end\\".into(),
                    },
                )
                .unwrap();
            let i = Index::parse(&c.text);
            assert_eq!(i.tables[0].rows[row].cells.len(), 2);
            assert!(c.text.contains("|end\\ |"));
            assert!(
                c.text
                    .contains(if row == 0 { "|end\\ |b|" } else { "|end\\ |y|" })
            );
        }
    }
    #[test]
    fn slash_runs_in_header_and_body_match_authoritative_gfm_cell_ranges() {
        for count in 0..=6 {
            for row in [0, 2] {
                let value = format!("a{}|x", "\\".repeat(count));
                let header = if row == 0 { value.as_str() } else { "a" };
                let body = if row == 2 { value.as_str() } else { "x" };
                let columns = if count == 0 && row == 0 { 3 } else { 2 };
                let delimiter = if columns == 3 {
                    "|---|---|---|"
                } else {
                    "|---|---|"
                };
                let s = format!("|{header}|b|\n{delimiter}\n|{body}|y|\n");
                let i = Index::parse(&s);
                assert_eq!(i.tables.len(), 1, "slashrun={count} row={row}");
                assert_eq!(i.tables[0].columns, columns);
                assert_eq!(
                    i.tables[0].rows[row].cells.len(),
                    if count == 0 { 3 } else { 2 }
                );
                let mut ast_cells = 0;
                for (event, range) in Parser::new_ext(&s, Options::all()).into_offset_iter() {
                    if matches!(event, Event::Start(Tag::TableCell))
                        && range.start >= i.tables[0].rows[row].range.start
                        && range.start < i.tables[0].rows[row].range.end
                    {
                        ast_cells += 1;
                    }
                }
                // GFM pads/truncates body rows to header width; raw source keeps excess.
                assert_eq!(ast_cells, columns, "slashrun={count} row={row}");
            }
        }
    }
    #[test]
    fn deleting_down_to_one_column_keeps_gfm_borders_without_touching_surviving_cells() {
        for left in ["", "|"] {
            for right in ["", "|"] {
                for newline in ["\n", "\r\n"] {
                    for ending in ["", newline] {
                        for column in [0, 1] {
                            let s = format!(
                                "{left}甲 | b{right}{newline}{left}--- | ---{right}{newline}{left}🦀 | y{right}{ending}"
                            );
                            let i = Index::parse(&s);
                            let caret = if column == 0 {
                                s.find("b").unwrap()
                            } else {
                                s.find("🦀").unwrap()
                            };
                            let caret = char_at(&s, caret);
                            let change = i
                                .apply(
                                    &s,
                                    [caret, caret],
                                    Command::DeleteColumn { table: 0, column },
                                )
                                .unwrap();
                            let after = Index::parse(&change.text);
                            assert_eq!(after.tables.len(), 1, "{s:?} -> {:?}", change.text);
                            assert_eq!(after.tables[0].columns, 1);
                            assert_eq!(after.tables[0].rows.len(), 3);
                            assert_eq!(change.text.ends_with('\n'), !ending.is_empty());
                            assert_eq!(
                                change.text[byte_at(&change.text, change.selection[0])..]
                                    .chars()
                                    .next(),
                                Some(if column == 0 { 'b' } else { '🦀' })
                            );
                            assert_eq!(
                                change.text[after.tables[0].rows[2].cells[0].clone()].trim(),
                                if column == 0 { "y" } else { "🦀" }
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn replacement_end_is_an_untouched_right_boundary_in_both_selection_directions() {
        let source = "|a|b|\n|---|---|\n|x|y|";
        for selection in [[2, 2], [2, 1], [1, 2]] {
            let change = Index::parse(source)
                .apply(
                    source,
                    selection,
                    Command::Cell {
                        table: 0,
                        row: 0,
                        column: 0,
                        value: "long🦀".into(),
                    },
                )
                .unwrap();
            assert_eq!(
                change.selection,
                selection.map(|c| if c == 2 { 6 } else { 1 })
            );
            assert_eq!(change.text.chars().nth(6), Some('|'));
        }
    }
    #[test]
    fn clearing_edge_cells_keeps_table_shape_neighbor_bytes_and_empty_body_rows() {
        for left in ["", "|"] {
            for right in ["", "|"] {
                for row in [0, 2] {
                    for column in [0, 1] {
                        for value in ["", "   "] {
                            let source = format!(
                                "{left}a | **乙**{right}\r\n{left}--- | ---{right}\r\n{left}x | `🦀`{right}"
                            );
                            let index = Index::parse(&source);
                            let unaffected = index.tables[0].rows[row].cells[1 - column].clone();
                            let expected_neighbor = &source[unaffected];
                            let change = index
                                .apply(
                                    &source,
                                    [0, 0],
                                    Command::Cell {
                                        table: 0,
                                        row,
                                        column,
                                        value: value.into(),
                                    },
                                )
                                .unwrap();
                            let after = Index::parse(&change.text);
                            assert_eq!(after.tables.len(), 1, "{source:?} -> {:?}", change.text);
                            assert_eq!(after.tables[0].columns, 2);
                            assert_eq!(after.tables[0].rows.len(), 3);
                            assert_eq!(after.tables[0].rows[row].cells.len(), 2);
                            assert_eq!(
                                &change.text[after.tables[0].rows[row].cells[1 - column].clone()],
                                expected_neighbor
                            );
                            assert!(
                                change.text[after.tables[0].rows[row].cells[column].clone()]
                                    .trim()
                                    .is_empty()
                            );
                            assert!(!change.text.ends_with('\n'));
                            assert_eq!(change.text.matches("\r\n").count(), 2);
                        }
                    }
                }
            }
        }
        for left in ["", "|"] {
            for right in ["", "|"] {
                for row in [0, 2] {
                    for value in ["", " "] {
                        // A one-column header without either border is not GFM. Its body
                        // may legitimately omit both borders, and still must remain a row.
                        let header = if left.is_empty() && right.is_empty() {
                            "|a|".into()
                        } else {
                            format!("{left}a{right}")
                        };
                        let source = format!("{header}\n|---|\n{left}x{right}");
                        let change = Index::parse(&source)
                            .apply(
                                &source,
                                [0, 0],
                                Command::Cell {
                                    table: 0,
                                    row,
                                    column: 0,
                                    value: value.into(),
                                },
                            )
                            .unwrap();
                        let after = Index::parse(&change.text);
                        assert_eq!(after.tables.len(), 1, "{source:?} -> {:?}", change.text);
                        assert_eq!(after.tables[0].columns, 1);
                        assert_eq!(after.tables[0].rows.len(), 3);
                        assert_eq!(after.tables[0].rows[row].cells.len(), 1);
                        assert!(
                            change.text[after.tables[0].rows[row].cells[0].clone()]
                                .trim()
                                .is_empty()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn editing_edge_block_prefixes_never_reinterprets_table_rows_as_other_blocks() {
        for left in ["", "|"] {
            for right in ["", "|"] {
                for row in [0, 2] {
                    for column in [0, 1] {
                        for value in [
                            "# title",
                            "- item",
                            "```",
                            "[ref]: /url",
                            "<div>",
                            "---",
                            "> quote",
                        ] {
                            let source = format!(
                                "{left}a | **b**{right}\r\n{left}--- | ---{right}\r\n{left}x | `y`{right}"
                            );
                            let index = Index::parse(&source);
                            let neighbor =
                                &source[index.tables[0].rows[row].cells[1 - column].clone()];
                            let change = index
                                .apply(
                                    &source,
                                    [0, 0],
                                    Command::Cell {
                                        table: 0,
                                        row,
                                        column,
                                        value: value.into(),
                                    },
                                )
                                .unwrap();
                            let after = Index::parse(&change.text);
                            assert_eq!(after.tables.len(), 1, "{value:?}: {:?}", change.text);
                            assert_eq!(after.tables[0].columns, 2);
                            assert_eq!(
                                after.tables[0].rows.len(),
                                3,
                                "{value:?}: {:?}",
                                change.text
                            );
                            assert_eq!(after.tables[0].rows[row].cells.len(), 2);
                            assert_eq!(
                                &change.text[after.tables[0].rows[row].cells[1 - column].clone()],
                                neighbor
                            );
                            assert_eq!(
                                change.text[after.tables[0].rows[row].cells[column].clone()].trim(),
                                value
                            );
                        }
                    }
                }
            }
        }
    }
}
