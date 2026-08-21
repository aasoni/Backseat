use crate::model::{Cell, FileDiff, Hunk, Row, Tone};

/// Parse `git diff -U3` output for a single file into side-by-side rows.
///
/// Pairing rule: within a hunk, a run of removed lines followed by a run of
/// added lines pairs index-wise (`del[i]` beside `add[i]`); leftovers render
/// one-sided. Context lines flush any open runs and occupy both halves.
pub fn parse_unified(diff_text: &str) -> FileDiff {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut cur: Option<HunkBuilder> = None;

    for line in diff_text.split('\n') {
        if line.starts_with("@@") {
            if let Some(b) = cur.take() {
                hunks.push(b.finish());
            }
            cur = Some(HunkBuilder::new(line));
            continue;
        }
        let Some(b) = cur.as_mut() else { continue };
        match line.chars().next() {
            Some(' ') => b.context(&line[1..]),
            Some('-') => b.removed(&line[1..]),
            Some('+') => b.added(&line[1..]),
            Some('\\') => {} // "\ No newline at end of file"
            None => {}       // trailing blank from final newline
            _ => {}          // diff headers between hunks of a combined diff — ignore
        }
    }
    if let Some(b) = cur.take() {
        hunks.push(b.finish());
    }
    FileDiff { hunks }
}

struct HunkBuilder {
    header: String,
    rows: Vec<Row>,
    del_run: Vec<Cell>,
    add_run: Vec<Cell>,
    left_no: u32,
    right_no: u32,
}

impl HunkBuilder {
    fn new(header_line: &str) -> HunkBuilder {
        let (left_no, right_no) = parse_hunk_header(header_line).unwrap_or((1, 1));
        HunkBuilder {
            header: header_line.to_string(),
            rows: Vec::new(),
            del_run: Vec::new(),
            add_run: Vec::new(),
            left_no,
            right_no,
        }
    }

    fn context(&mut self, text: &str) {
        self.flush_runs();
        self.rows.push(Row {
            left: Some(Cell {
                number: self.left_no,
                text: text.to_string(),
                tone: Tone::Ctx,
            }),
            right: Some(Cell {
                number: self.right_no,
                text: text.to_string(),
                tone: Tone::Ctx,
            }),
        });
        self.left_no += 1;
        self.right_no += 1;
    }

    fn removed(&mut self, text: &str) {
        self.del_run.push(Cell {
            number: self.left_no,
            text: text.to_string(),
            tone: Tone::Del,
        });
        self.left_no += 1;
    }

    fn added(&mut self, text: &str) {
        self.add_run.push(Cell {
            number: self.right_no,
            text: text.to_string(),
            tone: Tone::Add,
        });
        self.right_no += 1;
    }

    fn flush_runs(&mut self) {
        let dels = std::mem::take(&mut self.del_run);
        let adds = std::mem::take(&mut self.add_run);
        let n = dels.len().max(adds.len());
        let mut dels = dels.into_iter();
        let mut adds = adds.into_iter();
        for _ in 0..n {
            self.rows.push(Row {
                left: dels.next(),
                right: adds.next(),
            });
        }
    }

    fn finish(mut self) -> Hunk {
        self.flush_runs();
        Hunk {
            header: self.header,
            rows: self.rows,
        }
    }
}

/// Extract the starting line numbers from `@@ -l[,c] +l[,c] @@ …`.
fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@ ")?;
    let mut it = rest.split(' ');
    let old = it.next()?.strip_prefix('-')?;
    let new = it.next()?.strip_prefix('+')?;
    let old_start = old.split(',').next()?.parse().ok()?;
    let new_start = new.split(',').next()?.parse().ok()?;
    Some((old_start, new_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/foo.txt b/foo.txt
index 000..111 100644
--- a/foo.txt
+++ b/foo.txt
@@ -1,5 +1,6 @@ fn main
 line one
-old two
-old three
+new two
 line four
+extra five
 line six
\\ No newline at end of file
";

    #[test]
    fn pairs_del_add_runs() {
        let d = parse_unified(SAMPLE);
        assert_eq!(d.hunks.len(), 1);
        let rows = &d.hunks[0].rows;
        // ctx, (del+add pair), (del only), ctx, (add only), ctx
        assert_eq!(rows.len(), 6);

        assert_eq!(rows[0].left.as_ref().unwrap().number, 1);
        assert_eq!(rows[0].right.as_ref().unwrap().number, 1);
        assert_eq!(rows[0].left.as_ref().unwrap().tone, Tone::Ctx);

        // del "old two" (left 2) pairs with add "new two" (right 2)
        assert_eq!(rows[1].left.as_ref().unwrap().text, "old two");
        assert_eq!(rows[1].right.as_ref().unwrap().text, "new two");
        assert_eq!(rows[1].left.as_ref().unwrap().tone, Tone::Del);
        assert_eq!(rows[1].right.as_ref().unwrap().tone, Tone::Add);

        // leftover del renders one-sided
        assert_eq!(rows[2].left.as_ref().unwrap().text, "old three");
        assert!(rows[2].right.is_none());

        // ctx "line four": left 4, right 3
        assert_eq!(rows[3].left.as_ref().unwrap().number, 4);
        assert_eq!(rows[3].right.as_ref().unwrap().number, 3);

        // lone add
        assert!(rows[4].left.is_none());
        assert_eq!(rows[4].right.as_ref().unwrap().text, "extra five");
        assert_eq!(rows[4].right.as_ref().unwrap().number, 4);

        assert_eq!(rows[5].right.as_ref().unwrap().number, 5);
    }

    #[test]
    fn multiple_hunks() {
        let text = "@@ -1,2 +1,2 @@\n ctx\n-a\n+b\n@@ -10,2 +10,2 @@\n ctx2\n+only\n";
        let d = parse_unified(text);
        assert_eq!(d.hunks.len(), 2);
        assert_eq!(d.hunks[1].rows[1].right.as_ref().unwrap().number, 11);
    }

    #[test]
    fn empty_input() {
        assert!(parse_unified("").hunks.is_empty());
    }
}
