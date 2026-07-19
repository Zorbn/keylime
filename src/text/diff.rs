use crate::pool::{define_pool, Pooled};

#[derive(Debug, Clone, Copy)]
pub enum DiffEdit {
    Insert { b_index: usize, count: usize },
    Delete { count: usize },
    Substitute { b_index: usize, count: usize },
    Match { count: usize },
}

define_pool!(DIFF_EDIT_POOL, UTF16_POOL_ITEMS, Vec<DiffEdit>);

pub fn myers_diff<T: PartialEq>(a: &[T], b: &[T]) -> Pooled<Vec<DiffEdit>> {
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

    let mut edits = DIFF_EDIT_POOL.new_item();
    let mut i = a.len();
    let mut j = b.len();

    while i > 0 || j > 0 {
        let current = matrix[index(width, i, j)];

        let deletion = if i > 0 {
            matrix[index(width, i - 1, j)]
        } else {
            usize::MAX
        };

        let insertion = if j > 0 {
            matrix[index(width, i, j - 1)]
        } else {
            usize::MAX
        };

        let substitution = if i > 0 && j > 0 {
            matrix[index(width, i - 1, j - 1)]
        } else {
            usize::MAX
        };

        if deletion < insertion {
            if deletion < substitution {
                push_edit(&mut edits, DiffEdit::Delete { count: 1 });
                i -= 1;
            } else if substitution != current {
                push_edit(
                    &mut edits,
                    DiffEdit::Substitute {
                        b_index: j - 1,
                        count: 1,
                    },
                );
                i -= 1;
                j -= 1;
            } else {
                push_edit(&mut edits, DiffEdit::Match { count: 1 });
                i -= 1;
                j -= 1;
            }
        } else if insertion < substitution {
            push_edit(
                &mut edits,
                DiffEdit::Insert {
                    b_index: j - 1,
                    count: 1,
                },
            );
            j -= 1;
        } else if substitution != current {
            push_edit(
                &mut edits,
                DiffEdit::Substitute {
                    b_index: j - 1,
                    count: 1,
                },
            );
            i -= 1;
            j -= 1;
        } else {
            push_edit(&mut edits, DiffEdit::Match { count: 1 });
            i -= 1;
            j -= 1;
        }
    }

    edits.reverse();
    edits
}

fn push_edit(edits: &mut Vec<DiffEdit>, edit: DiffEdit) {
    if let Some(last_edit) = edits.last_mut() {
        match last_edit {
            DiffEdit::Insert { b_index, count } => {
                if let DiffEdit::Insert {
                    b_index: next_b_index,
                    ..
                } = edit
                {
                    if next_b_index == *b_index + 1 {
                        *count += 1;
                        return;
                    }
                }
            }
            DiffEdit::Delete { count } => {
                if let DiffEdit::Delete { .. } = edit {
                    *count += 1;
                    return;
                }
            }
            DiffEdit::Substitute { b_index, count } => {
                if let DiffEdit::Substitute {
                    b_index: next_b_index,
                    ..
                } = edit
                {
                    if next_b_index == *b_index + 1 {
                        *count += 1;
                        return;
                    }
                }
            }
            DiffEdit::Match { count } => {
                if let DiffEdit::Match { .. } = edit {
                    *count += 1;
                    return;
                }
            }
        }
    }

    edits.push(edit);
}

fn index(width: usize, i: usize, j: usize) -> usize {
    j + i * width
}
