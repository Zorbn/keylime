#[derive(Debug)]
pub enum DiffEdit {
    Insert { b_index: usize },
    Delete,
    Substite { b_index: usize },
    Match,
}

// TODO: Reuse memory, don't allocate on every diff.
pub fn myers_diff<T: PartialEq>(a: &[T], b: &[T]) -> Vec<DiffEdit> {
    let mut matrix = Vec::new();
    let height = a.len() + 1;
    let width = b.len() + 1;
    matrix.resize(height * width, 0);

    for i in 1..=b.len() {
        matrix[i] = i;
    }

    for i in 1..=a.len() {
        matrix[i * width] = i;
    }

    for i in 1..height {
        for j in 1..width {
            if a[i - 1] == b[j - 1] {
                matrix[index(width, i, j)] = matrix[index(width, i - 1, j - 1)];
                continue;
            }

            let deletion = matrix[index(width, i - 1, j)] + 1;
            let insertion = matrix[index(width, i, j - 1)] + 1;
            let substitution = matrix[index(width, i - 1, j - 1)] + 1;

            matrix[index(width, i, j)] = deletion.min(insertion).min(substitution);
        }
    }

    let mut edits = Vec::new();
    let mut i = a.len();
    let mut j = b.len();

    while i > 0 && j > 0 {
        let current = matrix[index(width, i, j)];

        let deletion = matrix[index(width, i - 1, j)];
        let insertion = matrix[index(width, i, j - 1)];
        let substitution = matrix[index(width, i - 1, j - 1)];

        if deletion < insertion {
            if deletion < substitution {
                edits.push(DiffEdit::Delete);
                i -= 1;
            } else if substitution != current {
                edits.push(DiffEdit::Substite { b_index: j - 1 });
                i -= 1;
                j -= 1;
            } else {
                edits.push(DiffEdit::Match);
                i -= 1;
                j -= 1;
            }
        } else if insertion < substitution {
            edits.push(DiffEdit::Insert { b_index: j - 1 });
            j -= 1;
        } else if substitution != current {
            edits.push(DiffEdit::Substite { b_index: j - 1 });
            i -= 1;
            j -= 1;
        } else {
            edits.push(DiffEdit::Match);
            i -= 1;
            j -= 1;
        }
    }

    edits.reverse();
    edits
}

fn index(width: usize, i: usize, j: usize) -> usize {
    j + i * width
}

fn build_from_edits(a: &str, b: &str, edits: &[DiffEdit]) -> String {
    let mut result = a.to_string();
    let mut a_index = 0;

    for edit in edits {
        match edit {
            DiffEdit::Delete => {
                result.remove(a_index);
            }
            DiffEdit::Insert { b_index } => {
                result.replace_range(a_index..a_index, &b[*b_index..*b_index + 1]);
                a_index += 1;
            }
            DiffEdit::Match => {
                a_index += 1;
            }
            DiffEdit::Substite { b_index } => {
                result.replace_range(a_index..a_index + 1, &b[*b_index..*b_index + 1]);
                a_index += 1;
            }
        }
    }

    result
}

#[test]
fn test_myers_diff() {
    let a = "hello world";
    let b = "hello there world";
    let edits = myers_diff(a.as_bytes(), b.as_bytes());

    println!("{:?}", edits);

    let result = build_from_edits(a, b, &edits);

    assert_eq!(result, b);

    let a = "hello world";
    let b = "hello rld";
    let edits = myers_diff(a.as_bytes(), b.as_bytes());
    let result = build_from_edits(a, b, &edits);

    println!("{:?}", edits);

    assert_eq!(result, b);
}

#[test]
fn test_myers_diff_lines() {
    let a = &[
        "fn main {",
        "    println!('Hello, world');",
        "}", //
    ];
    let b = &[
        "fn main {",
        "    println!('Hi, world');",
        "}", //
    ];
    let edits = myers_diff(a, b);

    println!("{:?}", edits);

    // let result = build_from_edits(a, b, &edits);

    // assert_eq!(result, b);
}
