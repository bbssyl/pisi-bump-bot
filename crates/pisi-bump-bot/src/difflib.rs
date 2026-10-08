use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub type MatchingBlock = (usize, usize, usize);
pub type Opcode = (&'static str, usize, usize, usize, usize);

pub struct SequenceMatcher<'a, T: Eq + Hash> {
    a: &'a [T],
    b: &'a [T],
    b2j: HashMap<&'a T, Vec<usize>>,
}

impl<'a, T: Eq + Hash> SequenceMatcher<'a, T> {
    pub fn new(a: &'a [T], b: &'a [T]) -> Self {
        let mut b2j: HashMap<&'a T, Vec<usize>> = HashMap::new();
        for (index, element) in b.iter().enumerate() {
            b2j.entry(element).or_default().push(index);
        }
        let autojunk_threshold = b.len() / 100 + 1;
        if b.len() >= 200 {
            let popular: HashSet<&'a T> = b2j
                .iter()
                .filter(|(_, indexes)| indexes.len() > autojunk_threshold)
                .map(|(element, _)| *element)
                .collect();
            for element in popular {
                b2j.remove(element);
            }
        }
        Self { a, b, b2j }
    }

    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> MatchingBlock {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut new_j2len: HashMap<usize, usize> = HashMap::new();
            if let Some(indexes) = self.b2j.get(&self.a[i]) {
                for &j in indexes {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let previous = if j > 0 {
                        *j2len.get(&(j - 1)).unwrap_or(&0)
                    } else {
                        0
                    };
                    let length = previous + 1;
                    new_j2len.insert(j, length);
                    if length > bestsize {
                        besti = i + 1 - length;
                        bestj = j + 1 - length;
                        bestsize = length;
                    }
                }
            }
            j2len = new_j2len;
        }
        while besti > alo && bestj > blo && self.a[besti - 1] == self.b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi
            && bestj + bestsize < bhi
            && self.a[besti + bestsize] == self.b[bestj + bestsize]
        {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    pub fn get_matching_blocks(&self) -> Vec<MatchingBlock> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut queue = vec![(0, la, 0, lb)];
        let mut raw_blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                raw_blocks.push((i, j, k));
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        raw_blocks.sort();
        collapse_adjacent_blocks(raw_blocks, la, lb)
    }

    pub fn ratio(&self) -> f64 {
        let matches: usize = self.get_matching_blocks().iter().map(|block| block.2).sum();
        let total = self.a.len() + self.b.len();
        if total == 0 {
            1.0
        } else {
            2.0 * matches as f64 / total as f64
        }
    }

    pub fn get_opcodes(&self) -> Vec<Opcode> {
        let (mut i, mut j) = (0, 0);
        let mut answer = Vec::new();
        for (ai, bj, size) in self.get_matching_blocks() {
            let tag = opcode_tag(i, ai, j, bj);
            if let Some(tag) = tag {
                answer.push((tag, i, ai, j, bj));
            }
            i = ai + size;
            j = bj + size;
            if size > 0 {
                answer.push(("equal", ai, i, bj, j));
            }
        }
        answer
    }

    pub fn get_grouped_opcodes(&self, context_lines: usize) -> Vec<Vec<Opcode>> {
        group_opcodes(self.get_opcodes(), context_lines)
    }
}

fn opcode_tag(i: usize, ai: usize, j: usize, bj: usize) -> Option<&'static str> {
    match (i < ai, j < bj) {
        (true, true) => Some("replace"),
        (true, false) => Some("delete"),
        (false, true) => Some("insert"),
        (false, false) => None,
    }
}

fn collapse_adjacent_blocks(
    blocks: Vec<MatchingBlock>,
    la: usize,
    lb: usize,
) -> Vec<MatchingBlock> {
    let (mut i1, mut j1, mut k1) = (0, 0, 0);
    let mut collapsed = Vec::new();
    for (i2, j2, k2) in blocks {
        if i1 + k1 == i2 && j1 + k1 == j2 {
            k1 += k2;
        } else {
            if k1 > 0 {
                collapsed.push((i1, j1, k1));
            }
            (i1, j1, k1) = (i2, j2, k2);
        }
    }
    if k1 > 0 {
        collapsed.push((i1, j1, k1));
    }
    collapsed.push((la, lb, 0));
    collapsed
}

fn clamp_leading_equal(code: Opcode, context_lines: usize) -> Opcode {
    let (tag, i1, i2, j1, j2) = code;
    (
        tag,
        i1.max(i2.saturating_sub(context_lines)),
        i2,
        j1.max(j2.saturating_sub(context_lines)),
        j2,
    )
}

fn clamp_trailing_equal(code: Opcode, context_lines: usize) -> Opcode {
    let (tag, i1, i2, j1, j2) = code;
    (
        tag,
        i1,
        i2.min(i1 + context_lines),
        j1,
        j2.min(j1 + context_lines),
    )
}

fn group_opcodes(mut codes: Vec<Opcode>, context_lines: usize) -> Vec<Vec<Opcode>> {
    if codes.is_empty() {
        codes = vec![("equal", 0, 1, 0, 1)];
    }
    if codes[0].0 == "equal" {
        codes[0] = clamp_leading_equal(codes[0], context_lines);
    }
    let last = codes.len() - 1;
    if codes[last].0 == "equal" {
        codes[last] = clamp_trailing_equal(codes[last], context_lines);
    }
    let mut groups = Vec::new();
    let mut group = Vec::new();
    for (tag, mut i1, i2, mut j1, j2) in codes {
        if tag == "equal" && i2 - i1 > 2 * context_lines {
            group.push((
                tag,
                i1,
                i2.min(i1 + context_lines),
                j1,
                j2.min(j1 + context_lines),
            ));
            groups.push(std::mem::take(&mut group));
            i1 = i1.max(i2.saturating_sub(context_lines));
            j1 = j1.max(j2.saturating_sub(context_lines));
        }
        group.push((tag, i1, i2, j1, j2));
    }
    if !group.is_empty() && !(group.len() == 1 && group[0].0 == "equal") {
        groups.push(group);
    }
    groups
}

fn format_range(start: usize, stop: usize) -> String {
    let length = stop - start;
    let beginning = start + 1;
    match length {
        1 => format!("{beginning}"),
        0 => format!("{},0", beginning - 1),
        _ => format!("{beginning},{length}"),
    }
}

fn emit_group_lines(out: &mut Vec<String>, a: &[&str], b: &[&str], group: &[Opcode]) {
    for &(tag, i1, i2, j1, j2) in group {
        if tag == "equal" {
            out.extend(a[i1..i2].iter().map(|line| format!(" {line}")));
            continue;
        }
        if tag == "replace" || tag == "delete" {
            out.extend(a[i1..i2].iter().map(|line| format!("-{line}")));
        }
        if tag == "replace" || tag == "insert" {
            out.extend(b[j1..j2].iter().map(|line| format!("+{line}")));
        }
    }
}

pub fn unified_diff(
    a: &[&str],
    b: &[&str],
    fromfile: &str,
    tofile: &str,
    context_lines: usize,
) -> Vec<String> {
    let matcher = SequenceMatcher::new(a, b);
    let mut out = Vec::new();
    for (index, group) in matcher
        .get_grouped_opcodes(context_lines)
        .into_iter()
        .enumerate()
    {
        if index == 0 {
            out.push(format!("--- {fromfile}\n"));
            out.push(format!("+++ {tofile}\n"));
        }
        let first = group[0];
        let last = group[group.len() - 1];
        out.push(format!(
            "@@ -{} +{} @@\n",
            format_range(first.1, last.2),
            format_range(first.3, last.4)
        ));
        emit_group_lines(&mut out, a, b, &group);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{SequenceMatcher, unified_diff};

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn should_return_one_when_sequences_are_identical() {
        let a = chars("abcdef");
        let b = chars("abcdef");
        assert_eq!(SequenceMatcher::new(&a, &b).ratio(), 1.0);
    }

    #[test]
    fn should_return_zero_when_sequences_share_nothing() {
        let a = chars("abc");
        let b = chars("xyz");
        assert_eq!(SequenceMatcher::new(&a, &b).ratio(), 0.0);
    }

    #[test]
    fn should_find_matching_blocks_with_terminator_when_sequences_overlap() {
        let a = chars("abxcd");
        let b = chars("abcd");
        let matcher = SequenceMatcher::new(&a, &b);
        let blocks = matcher.get_matching_blocks();
        assert_eq!(blocks.last(), Some(&(5, 4, 0)));
        let matched: usize = blocks.iter().map(|block| block.2).sum();
        assert_eq!(matched, 4);
    }

    #[test]
    fn should_produce_unified_diff_header_when_lines_differ() {
        let a = ["one\n", "two\n", "three\n"];
        let b = ["one\n", "two-changed\n", "three\n"];
        let diff = unified_diff(&a, &b, "a/file", "b/file", 3);
        assert_eq!(diff[0], "--- a/file\n");
        assert_eq!(diff[1], "+++ b/file\n");
        assert!(diff.iter().any(|line| line == "-two\n"));
        assert!(diff.iter().any(|line| line == "+two-changed\n"));
    }

    #[test]
    fn should_activate_autojunk_when_sequence_has_200_or_more_elements() {
        let mut long_b = vec!['x'; 199];
        long_b.push('a');
        let a = vec!['a'];
        let matcher = SequenceMatcher::new(&a, &long_b);
        assert!(
            matcher
                .get_matching_blocks()
                .iter()
                .any(|block| block.2 == 1)
        );
    }
}
